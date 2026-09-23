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
    LockExceedsBalance { have: u64, locked: u64, need: u64 },
    UnlockExceedsLocked { locked: u64, need: u64 },
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
    /// (neuron, token) → locked amount, always <= balance
    locked: BTreeMap<(NeuronId, TokenId), u64>,
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
        let locked = self.locked.get(&key).copied().unwrap_or(0);
        let unlocked = have - locked;
        if unlocked < amount {
            return Err(LedgerError::Insufficient { have: unlocked, need: amount });
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

    /// Move `amount` of `neuron`'s balance in `token` from liquid to locked
    /// (staking, property #26/#27's `v_ℓ ≠ 0` risk). Locking never mints,
    /// burns or moves balance between holders, so `check_token` stays
    /// invariant; it only shrinks what `burn` may touch.
    pub fn lock(&mut self, neuron: NeuronId, token: TokenId, amount: u64) -> Result<(), LedgerError> {
        let key = (neuron, token);
        let have = self.balances.get(&key).copied().unwrap_or(0);
        let locked = self.locked.get(&key).copied().unwrap_or(0);
        let new_locked = locked.saturating_add(amount);
        if new_locked > have {
            return Err(LedgerError::LockExceedsBalance { have, locked, need: amount });
        }
        *self.locked.entry(key).or_insert(0) = new_locked;
        Ok(())
    }

    /// Move `amount` back from locked to liquid.
    pub fn unlock(&mut self, neuron: NeuronId, token: TokenId, amount: u64) -> Result<(), LedgerError> {
        let key = (neuron, token);
        let locked = self.locked.get(&key).copied().unwrap_or(0);
        if amount > locked {
            return Err(LedgerError::UnlockExceedsLocked { locked, need: amount });
        }
        let remaining = locked - amount;
        if remaining == 0 {
            self.locked.remove(&key);
        } else {
            self.locked.insert(key, remaining);
        }
        Ok(())
    }

    pub fn locked_balance(&self, neuron: &NeuronId, token: &TokenId) -> u64 {
        self.locked.get(&(*neuron, *token)).copied().unwrap_or(0)
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
    fn lock_does_not_change_supply_or_balance() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.lock(a(), t(), 60).unwrap();
        assert!(led.check_token(t()));
        assert_eq!(led.supply(&t()), 100);
        assert_eq!(led.balance(&a(), &t()), 100);
        assert_eq!(led.locked_balance(&a(), &t()), 60);
    }

    #[test]
    fn lock_exceeding_balance_is_rejected() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        let err = led.lock(a(), t(), 101).unwrap_err();
        assert_eq!(err, LedgerError::LockExceedsBalance { have: 100, locked: 0, need: 101 });
        assert_eq!(led.locked_balance(&a(), &t()), 0);
    }

    #[test]
    fn lock_is_additive_across_calls() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.lock(a(), t(), 30).unwrap();
        led.lock(a(), t(), 30).unwrap();
        assert_eq!(led.locked_balance(&a(), &t()), 60);
        let err = led.lock(a(), t(), 41).unwrap_err();
        assert_eq!(err, LedgerError::LockExceedsBalance { have: 100, locked: 60, need: 41 });
    }

    #[test]
    fn unlock_returns_to_liquid() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.lock(a(), t(), 60).unwrap();
        led.unlock(a(), t(), 25).unwrap();
        assert_eq!(led.locked_balance(&a(), &t()), 35);
        // now burnable up to the newly-unlocked amount
        led.burn(a(), t(), 65).unwrap();
        assert!(led.check_token(t()));
        assert_eq!(led.supply(&t()), 35);
    }

    #[test]
    fn unlock_exceeding_locked_is_rejected() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.lock(a(), t(), 40).unwrap();
        let err = led.unlock(a(), t(), 41).unwrap_err();
        assert_eq!(err, LedgerError::UnlockExceedsLocked { locked: 40, need: 41 });
        assert_eq!(led.locked_balance(&a(), &t()), 40);
    }

    #[test]
    fn burn_cannot_touch_locked_balance() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100)]).unwrap();
        led.lock(a(), t(), 70).unwrap();
        // only 30 is unlocked; a burn of 31 must fail even though the raw
        // balance (100) would cover it
        let err = led.burn(a(), t(), 31).unwrap_err();
        assert_eq!(err, LedgerError::Insufficient { have: 30, need: 31 });
        assert!(led.check_token(t()));
        assert_eq!(led.balance(&a(), &t()), 100);
        assert_eq!(led.locked_balance(&a(), &t()), 70);
    }

    #[test]
    fn locked_balance_is_per_neuron_and_per_token() {
        let mut led = MintLedger::new();
        led.mint_batch(t(), &[(a(), 100), (b(), 100)]).unwrap();
        led.lock(a(), t(), 50).unwrap();
        assert_eq!(led.locked_balance(&a(), &t()), 50);
        assert_eq!(led.locked_balance(&b(), &t()), 0);
    }
}
