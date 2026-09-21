// ---
// tags: tok, rust, referral, plumb, sybil
// crystal-type: source
// crystal-domain: cyber
// ---
//! Property 33 — referring an inactive or Sybil account yields zero.
//!
//! [[cyber/launch]] core 5: "the referrer is paid in the referee's own
//! token, from the referee's own economy: a fake account has a book with
//! no fees and a token worth nothing, so referring it pays nothing... in
//! the spirit of paying on focus created and never per head."
//!
//! This models a referral payout as a conserved two-way split of a book's
//! activity-derived budget — reusing [`crate::conservation::conserve_and_allocate`]
//! rather than a new arithmetic path, so the split inherits that primitive's
//! existing conservation guarantee. `book_activity` stands in for whatever
//! upstream mechanism prices a book's real fee/registration activity (still
//! open — property 30's cybergraph naming-link registration half); an
//! inactive or fake book has zero activity by definition, so this closes
//! only the allocation half of Sybil-resistance. See `referral.rs` (launch
//! #32, not yet merged) for the birth-mint leg this composes with once both
//! land.

use tru::Fx;

use crate::conservation::conserve_and_allocate;

/// Referral payout split for one book: `referrer + newcomer == book_activity`
/// whenever `book_activity > 0`; both zero when it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferralPayout {
    pub referrer: u64,
    pub newcomer: u64,
}

/// Split `book_activity` tokens between referrer (share `r`) and newcomer
/// (share `1 - r`) under conservation. `book_activity == 0` — an inactive or
/// Sybil book with no real fees or registrations behind it — always yields
/// zero to both sides, for any `r`: there is no budget to allocate from.
pub fn referral_payout(referrer: [u8; 32], newcomer: [u8; 32], r: Fx, book_activity: u64) -> ReferralPayout {
    if book_activity == 0 {
        return ReferralPayout { referrer: 0, newcomer: 0 };
    }
    let r = if r < Fx::ZERO {
        Fx::ZERO
    } else if r > Fx::ONE {
        Fx::ONE
    } else {
        r
    };
    let raw = [(referrer, r), (newcomer, Fx::ONE - r)];
    match conserve_and_allocate(&raw, Fx::ONE, book_activity, book_activity) {
        Ok(out) => ReferralPayout {
            referrer: out[0].amount,
            newcomer: out[1].amount,
        },
        Err(_) => ReferralPayout { referrer: 0, newcomer: 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(b: u8) -> [u8; 32] {
        let mut x = [0u8; 32];
        x[0] = b;
        x
    }

    #[test]
    fn inactive_book_pays_nothing_at_any_share() {
        for r in [Fx::ZERO, Fx::from_ratio(1, 10), Fx::from_ratio(1, 2), Fx::ONE] {
            let out = referral_payout(h(1), h(2), r, 0);
            assert_eq!(out, ReferralPayout { referrer: 0, newcomer: 0 }, "r={r:?}");
        }
    }

    #[test]
    fn sybil_swarm_of_fake_books_yields_zero_aggregate_payout() {
        let r = Fx::from_ratio(1, 10);
        let total: u64 = (0..500u32)
            .map(|i| {
                let out = referral_payout(h(1), h((i % 255) as u8), r, 0);
                out.referrer + out.newcomer
            })
            .sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn genuine_book_splits_by_share_under_conservation() {
        // Mirrors referral.rs's own birth_mint fixture (r=1/10, total=1000):
        // 100 to the referrer, 900 home — same numbers, reached through the
        // shared conservation primitive instead of a duplicated formula.
        let out = referral_payout(h(1), h(2), Fx::from_ratio(1, 10), 1000);
        assert_eq!(out.referrer, 100);
        assert_eq!(out.newcomer, 900);
        assert_eq!(out.referrer + out.newcomer, 1000);
    }

    #[test]
    fn full_share_and_zero_share_are_still_conserved() {
        let full = referral_payout(h(1), h(2), Fx::ONE, 1000);
        assert_eq!(full, ReferralPayout { referrer: 1000, newcomer: 0 });

        let none = referral_payout(h(1), h(2), Fx::ZERO, 1000);
        assert_eq!(none, ReferralPayout { referrer: 0, newcomer: 1000 });
    }

    #[test]
    fn tiny_activity_still_conserves_no_leakage() {
        // A one-token book (e.g. a single spam registration) still conserves
        // exactly — no rounding leak that could be farmed at scale.
        for activity in [1u64, 2, 3, 7] {
            let out = referral_payout(h(1), h(2), Fx::from_ratio(1, 3), activity);
            assert_eq!(out.referrer + out.newcomer, activity, "activity={activity}");
        }
    }
}
