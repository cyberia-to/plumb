// ---
// tags: tok, rust, referral, plumb
// crystal-type: source
// crystal-domain: cyber
// ---
//! Pro-rata distribution of a personal book's fees to its $ν holders.
//!
//! launch.md property #32: "$ν holders receive the book's fees pro rata
//! under conservation." Every current holder of a book's $ν token is owed
//! `balance / supply` of the fee budget the book collected; this module
//! computes that split as an exact integer partition of `fees`, so the
//! caller's own token-conservation check (`MintLedger::check_token`) holds
//! after the mint legs it produces are applied.

use crate::ledger::{MintLedger, NeuronId, TokenId};

#[derive(Debug, PartialEq, Eq)]
pub enum FeeShareError {
    /// `nu_token` has no holders with a positive balance (supply is zero).
    NoSupply,
    /// Nothing to distribute.
    FeesZero,
}

/// One holder's pro-rata share of a book's fee budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeShare {
    pub holder: NeuronId,
    pub amount: u64,
}

/// Split `fees` (token units already collected by a book) pro rata across
/// every current holder of `nu_token`, weighted by ledger balance.
///
/// Holders are read directly off `ledger` (`MintLedger::holders_of`), so a
/// caller cannot under- or over-count them: `Σ amount == fees` exactly for
/// any positive `fees`, because `Σ balance == supply` is the ledger's own
/// conservation invariant. Integer division floors each share; the
/// remainder — at most `holders - 1` units, lost to flooring — goes to the
/// holder the ledger lists last (canonical `BTreeMap` order over
/// `(NeuronId, TokenId)`, i.e. ascending `NeuronId`).
pub fn distribute_fees(
    ledger: &MintLedger,
    nu_token: TokenId,
    fees: u64,
) -> Result<Vec<FeeShare>, FeeShareError> {
    if fees == 0 {
        return Err(FeeShareError::FeesZero);
    }
    let holders = ledger.holders_of(&nu_token);
    if holders.is_empty() {
        return Err(FeeShareError::NoSupply);
    }
    let supply: u128 = holders.iter().map(|(_, b)| *b as u128).sum();
    if supply == 0 {
        return Err(FeeShareError::NoSupply);
    }

    let mut out = Vec::with_capacity(holders.len());
    let mut allocated: u64 = 0;
    let last = holders.len() - 1;
    for (i, (holder, balance)) in holders.iter().enumerate() {
        let amount = if i == last {
            fees.saturating_sub(allocated)
        } else {
            let a = ((*balance as u128) * (fees as u128) / supply) as u64;
            allocated = allocated.saturating_add(a);
            a
        };
        out.push(FeeShare { holder: *holder, amount });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> TokenId {
        [9u8; 32]
    }
    fn n(b: u8) -> NeuronId {
        let mut x = [0u8; 32];
        x[0] = b;
        x
    }

    #[test]
    fn no_supply_is_an_error() {
        let ledger = MintLedger::new();
        assert_eq!(
            distribute_fees(&ledger, t(), 100),
            Err(FeeShareError::NoSupply)
        );
    }

    #[test]
    fn zero_fees_is_an_error() {
        let mut ledger = MintLedger::new();
        ledger.mint_batch(t(), &[(n(1), 100)]).unwrap();
        assert_eq!(
            distribute_fees(&ledger, t(), 0),
            Err(FeeShareError::FeesZero)
        );
    }

    #[test]
    fn sole_holder_takes_everything() {
        let mut ledger = MintLedger::new();
        ledger.mint_batch(t(), &[(n(1), 100)]).unwrap();
        let out = distribute_fees(&ledger, t(), 777).unwrap();
        assert_eq!(out, vec![FeeShare { holder: n(1), amount: 777 }]);
    }

    #[test]
    fn splits_exactly_by_balance_ratio() {
        let mut ledger = MintLedger::new();
        ledger.mint_batch(t(), &[(n(1), 75), (n(2), 25)]).unwrap();
        let out = distribute_fees(&ledger, t(), 1000).unwrap();
        let by = |h| out.iter().find(|s| s.holder == h).unwrap().amount;
        assert_eq!(by(n(1)), 750);
        assert_eq!(by(n(2)), 250);
    }

    #[test]
    fn sum_of_shares_conserves_fees_under_uneven_division() {
        let mut ledger = MintLedger::new();
        ledger
            .mint_batch(t(), &[(n(1), 7), (n(2), 11), (n(3), 13)])
            .unwrap();
        for fees in [1u64, 2, 3, 10, 31, 999, 1_000_003] {
            let out = distribute_fees(&ledger, t(), fees).unwrap();
            let paid: u64 = out.iter().map(|s| s.amount).sum();
            assert_eq!(paid, fees, "fees={fees}");
        }
    }

    #[test]
    fn a_holder_with_zero_balance_is_excluded() {
        let mut ledger = MintLedger::new();
        ledger.mint_batch(t(), &[(n(1), 100)]).unwrap();
        ledger.burn(n(1), t(), 100).unwrap();
        ledger.mint_batch(t(), &[(n(2), 50)]).unwrap();
        let out = distribute_fees(&ledger, t(), 900).unwrap();
        assert_eq!(out, vec![FeeShare { holder: n(2), amount: 900 }]);
    }

    #[test]
    fn different_token_holders_are_not_mixed_in() {
        let mut ledger = MintLedger::new();
        ledger.mint_batch(t(), &[(n(1), 100)]).unwrap();
        ledger.mint_batch([8u8; 32], &[(n(2), 900)]).unwrap();
        let out = distribute_fees(&ledger, t(), 50).unwrap();
        assert_eq!(out, vec![FeeShare { holder: n(1), amount: 50 }]);
    }
}
