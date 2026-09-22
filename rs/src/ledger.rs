// ---
// tags: tok, rust, ledger, plumb
// crystal-type: source
// crystal-domain: cyber
// ---
//! Mint ledger — Token conservation: Σ balances = mints − burns per Coin class.

use std::collections::BTreeMap;

/// Coin class / token particle id.
pub type TokenId = [u8; 32];
pub use neuron_id::NeuronId;

#[derive(Debug, PartialEq, Eq)]
pub enum LedgerError {
    Insufficient { have: u64, need: u64 },
    BurnExceedsMint,
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

    pub fn burn(&mut self, neuron: NeuronId, token: TokenId, amount: u64) -> Result<(), LedgerError> {
        let key = (neuron, token);
        let have = self.balances.get(&key).copied().unwrap_or(0);
        if have < amount {
            return Err(LedgerError::Insufficient { have, need: amount });
        }
        *self.balances.get_mut(&key).unwrap() = have - amount;
        *self.burned.entry(token).or_insert(0) =
            self.burned.get(&token).copied().unwrap_or(0).saturating_add(amount);
        if !self.check_token(token) {
            // restore
            *self.balances.get_mut(&key).unwrap() = have;
            *self.burned.get_mut(&token).unwrap() -= amount;
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

    pub fn supply(&self, token: &TokenId) -> u64 {
        self.total_minted(token)
            .saturating_sub(self.total_burned(token))
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
    fn burn_more_than_balance_is_rejected_and_state_unchanged() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        let err = led.burn(a(), t(), 150).unwrap_err();
        assert_eq!(err, LedgerError::Insufficient { have: 100, need: 150 });
        // rejected burn must not touch balance or the burned counter
        assert_eq!(led.balance(&a(), &t()), 100);
        assert_eq!(led.total_burned(&t()), 0);
        assert!(led.check_token(t()));
    }

    #[test]
    fn burn_from_untouched_neuron_is_rejected() {
        let mut led = MintLedger::new();
        let err = led.burn(a(), t(), 1).unwrap_err();
        assert_eq!(err, LedgerError::Insufficient { have: 0, need: 1 });
    }

    #[test]
    fn mint_batch_skips_zero_amount_legs() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100), (b(), 0)]).unwrap();
        assert_eq!(led.balance(&a(), &t()), 100);
        assert_eq!(led.balance(&b(), &t()), 0);
        assert_eq!(led.supply(&t()), 100);
    }

    #[test]
    fn mint_batch_of_all_zero_legs_is_a_no_op() {
        let mut led = MintLedger::new();
        let total = led.mint_batch(t(), &[(a(), 0), (b(), 0)]).unwrap();
        assert_eq!(total, 0);
        assert_eq!(led.supply(&t()), 0);
        assert_eq!(led.balance(&a(), &t()), 0);
    }

    #[test]
    fn mint_batch_of_empty_legs_is_a_no_op() {
        let mut led = MintLedger::new();
        let total = led.mint_batch(t(), &[]).unwrap();
        assert_eq!(total, 0);
        assert_eq!(led.supply(&t()), 0);
    }

    #[test]
    fn balances_are_isolated_per_token() {
        let mut led = MintLedger::new();
        let other_token = [9u8; 32];
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.mint_batch(other_token, &[(a(), 5)]).unwrap();
        assert_eq!(led.balance(&a(), &t()), 100);
        assert_eq!(led.balance(&a(), &other_token), 5);
        led.burn(a(), other_token, 5).unwrap();
        // burning the second token must not touch the first token's balance
        assert_eq!(led.balance(&a(), &t()), 100);
        assert_eq!(led.supply(&t()), 100);
    }

    #[test]
    fn repeated_neuron_in_one_batch_accumulates() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 30), (a(), 20)]).unwrap();
        assert_eq!(led.balance(&a(), &t()), 50);
        assert_eq!(led.supply(&t()), 50);
        assert!(led.check_token(t()));
    }

    #[test]
    fn untouched_token_and_neuron_default_to_zero() {
        let led = MintLedger::new();
        assert_eq!(led.balance(&a(), &t()), 0);
        assert_eq!(led.total_minted(&t()), 0);
        assert_eq!(led.total_burned(&t()), 0);
        assert_eq!(led.supply(&t()), 0);
        assert!(led.check_token(t()));
    }
}
