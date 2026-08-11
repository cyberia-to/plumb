// ---
// tags: tok, rust, ledger, plumb
// crystal-type: source
// crystal-domain: cyber
// ---
//! Mint ledger — Token conservation: Σ balances = mints − burns per Coin class.

use std::collections::BTreeMap;

/// Coin class / token particle id.
pub type TokenId = [u8; 32];
pub type NeuronId = [u8; 32];

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
}
