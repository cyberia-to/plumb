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
    if let Some(ref_neuron) = referrer {
        if referrer_amount > 0 {
            legs.push((ref_neuron, referrer_amount));
        }
    }
    ledger.mint_batch(token, &legs)?;
    Ok(BirthMintReceipt {
        neuron_amount,
        referrer_amount,
    })
}

/// `round(total * r)`, clamped so the referrer never receives more than the
/// birth mint itself even if `r` is misconfigured above 1.
fn referral_share(total: u64, r: Fx) -> u64 {
    if r <= Fx::ZERO {
        return 0;
    }
    let f = r.to_f64().clamp(0.0, 1.0);
    ((total as f64) * f).round() as u64
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
}
