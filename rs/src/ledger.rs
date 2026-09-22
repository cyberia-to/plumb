// ---
// tags: tok, rust, ledger, plumb
// crystal-type: source
// crystal-domain: cyber
// ---
//! Mint ledger — Token conservation: Σ balances = mints − burns per Coin class.

use std::collections::BTreeMap;

use cyber_hemera::hash as hemera_hash;

/// Coin class / token particle id.
pub type TokenId = [u8; 32];
pub use neuron_id::NeuronId;

#[derive(Debug, PartialEq, Eq)]
pub enum LedgerError {
    Insufficient { have: u64, need: u64 },
    BurnExceedsMint,
    /// The credited balance would exceed `u64::MAX`; cannot happen while
    /// Σ balances = minted − burned holds, rejected rather than saturated.
    Overflow,
    /// Σ balances ≠ minted − burned after the mutation; state rolled back.
    Conservation,
}

/// Tracks global mint/burn and per-holder balances for conservation checks.
#[derive(Clone, Debug, Default)]
pub struct MintLedger {
    /// token → total minted
    minted: BTreeMap<TokenId, u64>,
    /// token → total burned
    burned: BTreeMap<TokenId, u64>,
    /// (neuron, token) → balance
    balances: BTreeMap<(NeuronId, TokenId), u64>,
}

impl MintLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply mint legs atomically. All succeed or none (caller rolls back by not keeping ledger).
    pub fn mint_batch(
        &mut self,
        token: TokenId,
        legs: &[(NeuronId, u64)],
    ) -> Result<u64, LedgerError> {
        let total: u64 = legs.iter().map(|(_, a)| *a).sum();
        if total == 0 {
            return Ok(0);
        }
        // Snapshot for atomicity
        let snap_minted = self.minted.clone();
        let snap_balances = self.balances.clone();
        for (neuron, amount) in legs {
            if *amount == 0 {
                continue;
            }
            *self.minted.entry(token).or_insert(0) =
                self.minted.get(&token).copied().unwrap_or(0).saturating_add(*amount);
            let key = (*neuron, token);
            *self.balances.entry(key).or_insert(0) =
                self.balances.get(&key).copied().unwrap_or(0).saturating_add(*amount);
        }
        // Conservation check
        if !self.check_token(token) {
            self.minted = snap_minted;
            self.balances = snap_balances;
            return Err(LedgerError::BurnExceedsMint);
        }
        Ok(total)
    }

    /// Move balance between two holders of the same token. Debits `from`,
    /// credits `to`; `minted`/`burned` are untouched, so Σ balances must be
    /// unchanged. Property 13 (`launch.md`) asks for the invariant to be
    /// checked at every mutation, so the check runs here too: on failure the
    /// two balances are restored and `LedgerError::Conservation` returned.
    pub fn transfer(
        &mut self,
        from: NeuronId,
        to: NeuronId,
        token: TokenId,
        amount: u64,
    ) -> Result<(), LedgerError> {
        if amount == 0 || from == to {
            return Ok(());
        }
        let from_key = (from, token);
        let have = self.balances.get(&from_key).copied().unwrap_or(0);
        if have < amount {
            return Err(LedgerError::Insufficient { have, need: amount });
        }
        let to_key = (to, token);
        let to_have = self.balances.get(&to_key).copied().unwrap_or(0);
        let to_new = to_have.checked_add(amount).ok_or(LedgerError::Overflow)?;
        self.balances.insert(from_key, have - amount);
        self.balances.insert(to_key, to_new);
        if !self.check_token(token) {
            self.balances.insert(from_key, have);
            self.balances.insert(to_key, to_have);
            return Err(LedgerError::Conservation);
        }
        Ok(())
    }

    pub fn burn(&mut self, neuron: NeuronId, token: TokenId, amount: u64) -> Result<(), LedgerError> {
        if amount == 0 {
            return Ok(());
        }
        let key = (neuron, token);
        let have = self.balances.get(&key).copied().unwrap_or(0);
        if have < amount {
            return Err(LedgerError::Insufficient { have, need: amount });
        }
        let burned_before = self.total_burned(&token);
        self.balances.insert(key, have - amount);
        self.burned.insert(token, burned_before.saturating_add(amount));
        if !self.check_token(token) {
            // restore
            self.balances.insert(key, have);
            self.burned.insert(token, burned_before);
            return Err(LedgerError::BurnExceedsMint);
        }
        Ok(())
    }

    pub fn balance(&self, neuron: &NeuronId, token: &TokenId) -> u64 {
        self.balances
            .get(&(*neuron, *token))
            .copied()
            .unwrap_or(0)
    }

    pub fn total_minted(&self, token: &TokenId) -> u64 {
        self.minted.get(token).copied().unwrap_or(0)
    }

    pub fn total_burned(&self, token: &TokenId) -> u64 {
        self.burned.get(token).copied().unwrap_or(0)
    }

    /// Σ balances == minted − burned.
    pub fn check_token(&self, token: TokenId) -> bool {
        let sum_bal: u64 = self
            .balances
            .iter()
            .filter(|((_, t), _)| *t == token)
            .map(|(_, b)| *b)
            .sum();
        let m = self.total_minted(&token);
        let b = self.total_burned(&token);
        sum_bal == m.saturating_sub(b)
    }

    /// Every holder of `token` with a nonzero balance, ascending by neuron id.
    pub fn holders(&self, token: &TokenId) -> Vec<(NeuronId, u64)> {
        self.balances
            .iter()
            .filter(|((_, t), b)| t == token && **b > 0)
            .map(|((n, _), b)| (*n, *b))
            .collect()
    }

    pub fn supply(&self, token: &TokenId) -> u64 {
        self.total_minted(token)
            .saturating_sub(self.total_burned(token))
    }
}

/// oikos home-book roots: one neuron, one book token, phase 1 scope.
///
/// `book_token` derives a token id deterministically from the neuron id, so
/// two neurons can never collide on a book and the same neuron always finds
/// the same book. `root` is the one-time registration: a neuron may root
/// its home book once. See `oikos-book.md` and `cyber/research/oikos.md`.
#[derive(Debug, PartialEq, Eq)]
pub enum BookError {
    AlreadyRooted,
}

#[derive(Clone, Debug, Default)]
pub struct BookRegistry {
    /// neuron → its home book's token id, set once at root().
    roots: BTreeMap<NeuronId, TokenId>,
}

impl BookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deterministic book token id for a neuron: hemera(domain-tag ‖ neuron).
    /// Pure function — does not require a prior root() call.
    pub fn book_token(neuron: &NeuronId) -> TokenId {
        let mut buf = Vec::with_capacity(13 + 32);
        buf.extend_from_slice(b"oikos-book-v0");
        buf.extend_from_slice(neuron);
        *hemera_hash(&buf)
            .as_bytes()
            .first_chunk::<32>()
            .unwrap_or(&[0u8; 32])
    }

    /// Root a neuron's home book. Fails if this neuron already rooted one —
    /// phase 1 gives every neuron exactly one home book.
    pub fn root(&mut self, neuron: NeuronId) -> Result<TokenId, BookError> {
        if self.roots.contains_key(&neuron) {
            return Err(BookError::AlreadyRooted);
        }
        let token = Self::book_token(&neuron);
        self.roots.insert(neuron, token);
        Ok(token)
    }

    /// The home book token a neuron already rooted, if any.
    pub fn book_of(&self, neuron: &NeuronId) -> Option<TokenId> {
        self.roots.get(neuron).copied()
    }

    pub fn is_rooted(&self, neuron: &NeuronId) -> bool {
        self.roots.contains_key(neuron)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> TokenId {
        [7u8; 32]
    }
    fn a() -> NeuronId {
        [1u8; 32]
    }
    fn b() -> NeuronId {
        [2u8; 32]
    }

    #[test]
    fn mint_preserves_conservation() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100), (b(), 50)]).unwrap();
        assert!(led.check_token(t()));
        assert_eq!(led.supply(&t()), 150);
        assert_eq!(led.balance(&a(), &t()), 100);
    }

    #[test]
    fn burn_preserves_conservation() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.burn(a(), t(), 40).unwrap();
        assert!(led.check_token(t()));
        assert_eq!(led.supply(&t()), 60);
    }

    #[test]
    fn book_token_is_deterministic_and_distinct() {
        assert_eq!(BookRegistry::book_token(&a()), BookRegistry::book_token(&a()));
        assert_ne!(BookRegistry::book_token(&a()), BookRegistry::book_token(&b()));
    }

    #[test]
    fn root_returns_the_deterministic_book_token() {
        let mut reg = BookRegistry::new();
        let token = reg.root(a()).unwrap();
        assert_eq!(token, BookRegistry::book_token(&a()));
        assert_eq!(reg.book_of(&a()), Some(token));
    }

    #[test]
    fn a_neuron_roots_its_home_book_once() {
        let mut reg = BookRegistry::new();
        reg.root(a()).unwrap();
        assert_eq!(reg.root(a()), Err(BookError::AlreadyRooted));
    }

    #[test]
    fn two_neurons_root_two_distinct_books() {
        let mut reg = BookRegistry::new();
        let ta = reg.root(a()).unwrap();
        let tb = reg.root(b()).unwrap();
        assert_ne!(ta, tb);
        assert!(reg.is_rooted(&a()));
        assert!(reg.is_rooted(&b()));
    }

    #[test]
    fn unrooted_neuron_has_no_book() {
        let reg = BookRegistry::new();
        assert_eq!(reg.book_of(&a()), None);
        assert!(!reg.is_rooted(&a()));
    }

    #[test]
    fn transfer_preserves_conservation() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        let supply_before = led.supply(&t());
        led.transfer(a(), b(), t(), 30).unwrap();
        assert!(led.check_token(t()));
        assert_eq!(led.supply(&t()), supply_before);
        assert_eq!(led.balance(&a(), &t()), 70);
        assert_eq!(led.balance(&b(), &t()), 30);
    }

    #[test]
    fn transfer_rejects_insufficient_balance() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 10)]).unwrap();
        let err = led.transfer(a(), b(), t(), 11).unwrap_err();
        assert_eq!(err, LedgerError::Insufficient { have: 10, need: 11 });
        // untouched on rejection
        assert_eq!(led.balance(&a(), &t()), 10);
        assert_eq!(led.balance(&b(), &t()), 0);
        assert!(led.check_token(t()));
    }

    #[test]
    fn transfer_zero_and_self_are_noops() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 50)]).unwrap();
        led.transfer(a(), b(), t(), 0).unwrap();
        led.transfer(a(), a(), t(), 50).unwrap();
        assert_eq!(led.balance(&a(), &t()), 50);
        assert_eq!(led.balance(&b(), &t()), 0);
        assert!(led.check_token(t()));
    }

    #[test]
    fn transfer_chain_conserves_total_across_holders() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.transfer(a(), b(), t(), 60).unwrap();
        led.transfer(b(), a(), t(), 25).unwrap();
        assert_eq!(led.balance(&a(), &t()), 65);
        assert_eq!(led.balance(&b(), &t()), 35);
        assert_eq!(led.supply(&t()), 100);
        assert!(led.check_token(t()));
    }
    #[test]
    fn conservation_holds_after_every_mutation_in_a_random_sequence() {
        // Deterministic LCG drives 2000 mixed mint/burn/transfer steps over
        // four holders; the invariant Σ balances = minted − burned is asserted
        // after every step, and rejected steps leave the state untouched.
        let holders: [NeuronId; 4] = [[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
        let mut led = MintLedger::new();
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            x >> 33
        };
        for _ in 0..2000 {
            let op = next() % 3;
            let i = (next() % 4) as usize;
            let j = (next() % 4) as usize;
            let amt = next() % 1000;
            let before = led.clone();
            let res = match op {
                0 => led.mint_batch(t(), &[(holders[i], amt), (holders[j], amt / 2)]).map(|_| ()),
                1 => led.burn(holders[i], t(), amt),
                _ => led.transfer(holders[i], holders[j], t(), amt),
            };
            assert!(led.check_token(t()), "op={op} i={i} j={j} amt={amt}");
            if res.is_err() {
                assert_eq!(led.balances, before.balances, "rejected op mutated balances");
                assert_eq!(led.minted, before.minted);
                assert_eq!(led.burned, before.burned);
            }
        }
        let sum: u64 = holders.iter().map(|h| led.balance(h, &t())).sum();
        assert_eq!(sum, led.supply(&t()));
    }
    #[test]
    fn zero_burn_on_an_unknown_holder_is_a_noop() {
        let mut led = MintLedger::new();
        led.burn(a(), t(), 0).unwrap();
        assert_eq!(led.burn(a(), t(), 1), Err(LedgerError::Insufficient { have: 0, need: 1 }));
        assert!(led.check_token(t()));
    }
}
