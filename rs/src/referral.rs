// ---
// tags: tok, rust, referral, plumb, oikos
// crystal-type: source
// crystal-domain: cyber
// ---
//! Referral birth allocation — property 32: a share `r` of a book's birth
//! mint goes to the referrer whose cyberlink named the newcomer; the rest
//! mints home to the neuron the book belongs to.
//!
//! `r` is a genesis parameter, not fixed here — see `launch.md` decisions
//! log, "the referral share r" is still open.

use tru::arithmetic::FRAC_BITS;
use tru::Fx;

use crate::ledger::{LedgerError, MintLedger, NeuronId, TokenId};

#[derive(Debug, PartialEq, Eq)]
pub enum ReferralError {
    /// A neuron cannot be its own referrer.
    SelfReferral,
    Ledger(LedgerError),
}

impl From<LedgerError> for ReferralError {
    fn from(e: LedgerError) -> Self {
        ReferralError::Ledger(e)
    }
}

/// Split of a book's birth mint between the neuron and its referrer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BirthMintReceipt {
    pub neuron_amount: u64,
    pub referrer_amount: u64,
}

/// Mint a book's birth supply of `total` tokens. With a referrer, a share
/// `r` of `total` mints to the referrer and the rest to the neuron; with no
/// referrer (self-registration, no referral cyberlink) the whole amount
/// mints home. Conservation holds by construction: `mint_batch` commits both
/// legs atomically.
pub fn birth_mint(
    ledger: &mut MintLedger,
    token: TokenId,
    neuron: NeuronId,
    referrer: Option<NeuronId>,
    total: u64,
    r: Fx,
) -> Result<BirthMintReceipt, ReferralError> {
    if referrer == Some(neuron) {
        return Err(ReferralError::SelfReferral);
    }
    let referrer_amount = match referrer {
        Some(_) => referral_share(total, r),
        None => 0,
    };
    let neuron_amount = total - referrer_amount;

    let mut legs: Vec<(NeuronId, u64)> = Vec::with_capacity(2);
    if neuron_amount > 0 {
        legs.push((neuron, neuron_amount));
    }
    if let Some(ref_neuron) = referrer.filter(|_| referrer_amount > 0) {
        legs.push((ref_neuron, referrer_amount));
    }
    ledger.mint_batch(token, &legs)?;
    Ok(BirthMintReceipt {
        neuron_amount,
        referrer_amount,
    })
}

/// `floor(total · r)` in fixed point: `r` is carried as `round(r · 2^FRAC_BITS)`
/// (`tru/specs/arithmetic.md`), so the product is an exact u128 multiply and
/// a shift — no float on the path. `r ≤ 0` pays nothing; `r ≥ 1` is clamped
/// so the referrer never receives more than the birth mint itself.
fn referral_share(total: u64, r: Fx) -> u64 {
    if r <= Fx::ZERO {
        return 0;
    }
    if r >= Fx::ONE {
        return total;
    }
    let scaled = r.to_i64_scaled(FRAC_BITS) as u128; // in (0, 2^FRAC_BITS)
    ((total as u128 * scaled) >> FRAC_BITS) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> TokenId {
        [9u8; 32]
    }
    fn newcomer() -> NeuronId {
        [3u8; 32]
    }
    fn referrer() -> NeuronId {
        [4u8; 32]
    }

    #[test]
    fn birth_mint_splits_by_referral_share() {
        let mut led = MintLedger::new();
        let rec = birth_mint(&mut led, t(), newcomer(), Some(referrer()), 1000, Fx::from_ratio(1, 10))
            .unwrap();
        assert_eq!(rec.referrer_amount, 100);
        assert_eq!(rec.neuron_amount, 900);
        assert!(led.check_token(t()));
        assert_eq!(led.supply(&t()), 1000);
        assert_eq!(led.balance(&referrer(), &t()), 100);
        assert_eq!(led.balance(&newcomer(), &t()), 900);
    }

    #[test]
    fn birth_mint_with_no_referrer_pays_home_in_full() {
        let mut led = MintLedger::new();
        let rec = birth_mint(&mut led, t(), newcomer(), None, 1000, Fx::from_ratio(1, 10)).unwrap();
        assert_eq!(rec.referrer_amount, 0);
        assert_eq!(rec.neuron_amount, 1000);
        assert_eq!(led.balance(&newcomer(), &t()), 1000);
    }

    #[test]
    fn self_referral_is_rejected_and_mints_nothing() {
        let mut led = MintLedger::new();
        let err = birth_mint(&mut led, t(), newcomer(), Some(newcomer()), 1000, Fx::from_ratio(1, 10))
            .unwrap_err();
        assert_eq!(err, ReferralError::SelfReferral);
        assert_eq!(led.supply(&t()), 0);
    }

    #[test]
    fn zero_share_pays_the_referrer_nothing() {
        let mut led = MintLedger::new();
        let rec = birth_mint(&mut led, t(), newcomer(), Some(referrer()), 1000, Fx::ZERO).unwrap();
        assert_eq!(rec.referrer_amount, 0);
        assert_eq!(rec.neuron_amount, 1000);
    }

    #[test]
    fn full_share_pays_the_referrer_everything() {
        let mut led = MintLedger::new();
        let rec = birth_mint(&mut led, t(), newcomer(), Some(referrer()), 1000, Fx::ONE).unwrap();
        assert_eq!(rec.referrer_amount, 1000);
        assert_eq!(rec.neuron_amount, 0);
        assert!(led.check_token(t()));
    }
    #[test]
    fn share_above_one_is_clamped_to_the_whole_mint() {
        let mut led = MintLedger::new();
        let rec = birth_mint(&mut led, t(), newcomer(), Some(referrer()), 1000, Fx::from_int(3)).unwrap();
        assert_eq!(rec, BirthMintReceipt { neuron_amount: 0, referrer_amount: 1000 });
        assert!(led.check_token(t()));
    }

    #[test]
    fn split_is_conserved_and_floored_across_a_sweep() {
        // Every (total, r) pair mints exactly `total`, the referrer's leg is
        // floor(total · r) — never more than the share promised — and the
        // ledger's own conservation check holds after the batch.
        for total in [1u64, 2, 3, 7, 99, 1000, 123_456_789, u64::MAX / 4] {
            for (num, den) in [(1, 10), (1, 3), (2, 3), (1, 7), (999, 1000), (1, 1_000_000)] {
                let r = Fx::from_ratio(num, den);
                let mut led = MintLedger::new();
                let rec = birth_mint(&mut led, t(), newcomer(), Some(referrer()), total, r).unwrap();
                assert_eq!(rec.neuron_amount + rec.referrer_amount, total, "total={total} r={num}/{den}");
                assert_eq!(led.supply(&t()), total);
                assert!(led.check_token(t()));
                // floor(total·num/den) up to r's own 2^-32 rounding, which
                // scales with total: tolerance (total >> 32) + 1.
                let exact = (total as u128 * num as u128 / den as u128) as u64;
                let tol = (total >> 32) + 1;
                assert!(
                    rec.referrer_amount + tol >= exact && rec.referrer_amount <= exact + tol,
                    "total={total} r={num}/{den} got={} exact={exact}",
                    rec.referrer_amount
                );
            }
        }
    }
}
