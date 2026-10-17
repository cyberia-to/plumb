// ---
// tags: tok, rust, staking, rewards
// crystal-type: source
// crystal-domain: cyber
// ---
//! Stake yield — the fourth term of the reward equation (`tru/specs/rewards.md`
//! §11): `R_PoS · (a_ν κ(ν)) / Σ_μ (a_μ κ(μ))`.
//!
//! §9 splits stake onto two independent axes: any real stake, including
//! passive (`valence == 0`), moves rank in `A_eff`; only correct risk under
//! `valence != 0` earns a reward. This module enforces the reward half —
//! [`stake_yield_shares`] excludes every passive position from both the
//! numerator and the denominator, so passive or Sybil-split stake draws zero
//! from the PoS pool no matter how large it is, while active stake shares the
//! pool in proportion to `amount · karma`.

use neuron_id::NeuronId;
use tru::Fx;

/// One neuron's staking position for an epoch's stake-yield pass.
#[derive(Clone, Copy, Debug)]
pub struct StakePosition {
    pub neuron: NeuronId,
    /// Staked amount, in token base units.
    pub amount: u128,
    /// Risk exposure: `0` passive (rank only, §9), `±1` active (risk, earns).
    pub valence: i8,
    /// Karma weight on active stake (§11) — reputation, not stake itself.
    pub karma: Fx,
}

impl StakePosition {
    fn is_active(&self) -> bool {
        self.valence != 0
    }
}

/// Split `pos_pool` (this epoch's `R_PoS`, §10) across `positions` by §11's
/// stake-yield term. A position with `valence == 0` always receives
/// [`Fx::ZERO`], regardless of `amount` — splitting one large passive stake
/// into many small ones (a Sybil position) still nets zero, since every
/// piece stays passive. Returns one share per input position, same order.
///
/// If no position is active, the whole pool goes unclaimed this epoch (every
/// share is zero) — §10 names no rule for that case; carrying `pos_pool`
/// forward is a caller decision, not this function's.
pub fn stake_yield_shares(positions: &[StakePosition], pos_pool: Fx) -> Vec<(NeuronId, Fx)> {
    if positions.is_empty() {
        return vec![];
    }

    // Normalize by the largest active amount so the weight stays in (0,1]
    // regardless of how large stake amounts get (same pattern tru's focusing
    // graph uses for stake weights) — the normalization cancels in the ratio.
    let max_active_amount = positions
        .iter()
        .filter(|p| p.is_active())
        .map(|p| p.amount)
        .max()
        .unwrap_or(1)
        .max(1);

    let weights: Vec<(NeuronId, Fx)> = positions
        .iter()
        .map(|p| {
            let w = if p.is_active() {
                Fx::ratio_u128(p.amount, max_active_amount) * p.karma
            } else {
                Fx::ZERO
            };
            (p.neuron, w)
        })
        .collect();

    let total = weights.iter().fold(Fx::ZERO, |acc, (_, w)| acc + *w);

    if total.is_zero() {
        return weights.into_iter().map(|(n, _)| (n, Fx::ZERO)).collect();
    }

    // Every rounded quotient `w / total` loses up to one ulp, so the shares
    // would not sum to `pos_pool` exactly. The largest active weight takes
    // the residual `pos_pool − Σ others` instead of its own rounded share:
    // the pool is conserved by construction, and the largest share is far
    // above the few ulps of dust the residual absorbs.
    let residual_at = weights
        .iter()
        .enumerate()
        .max_by(|(_, (_, a)), (_, (_, b))| a.cmp(b))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let mut shares: Vec<(NeuronId, Fx)> = weights
        .iter()
        .map(|(n, w)| (*n, pos_pool * w.div(total)))
        .collect();
    let others = shares
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != residual_at)
        .fold(Fx::ZERO, |acc, (_, (_, s))| acc + *s);
    shares[residual_at].1 = pos_pool - others;
    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(b: u8) -> NeuronId {
        [b; 32]
    }

    fn passive(neuron: u8, amount: u128) -> StakePosition {
        StakePosition { neuron: n(neuron), amount, valence: 0, karma: Fx::ONE }
    }

    fn active(neuron: u8, amount: u128, karma: Fx) -> StakePosition {
        StakePosition { neuron: n(neuron), amount, valence: 1, karma }
    }

    #[test]
    fn idle_capital_earns_nothing() {
        // A single, enormous passive stake — pure rank purchase, no risk.
        let positions = [passive(1, 1_000_000_000_000)];
        let shares = stake_yield_shares(&positions, Fx::from_int(1000));
        assert_eq!(shares, vec![(n(1), Fx::ZERO)]);
    }

    #[test]
    fn sybil_split_earns_nothing() {
        // The same idle capital split across many identities stays passive in
        // every piece — Sybil-splitting a purchase does not turn it into risk.
        let positions: Vec<StakePosition> = (1..=50).map(|i| passive(i, 10_000)).collect();
        let shares = stake_yield_shares(&positions, Fx::from_int(1000));
        assert!(shares.iter().all(|(_, s)| s.is_zero()));
    }

    #[test]
    fn no_active_stake_leaves_the_pool_unclaimed() {
        let positions = [passive(1, 500), passive(2, 500)];
        let shares = stake_yield_shares(&positions, Fx::from_int(1000));
        assert!(shares.iter().all(|(_, s)| s.is_zero()));
    }

    #[test]
    fn active_stake_shares_proportionally_by_amount_and_karma() {
        // Equal karma, amounts 300:100 → a 3:1 split of the pool.
        let positions = [active(1, 300, Fx::ONE), active(2, 100, Fx::ONE)];
        let shares = stake_yield_shares(&positions, Fx::from_int(100));
        assert_eq!(shares[0], (n(1), Fx::from_int(75)));
        assert_eq!(shares[1], (n(2), Fx::from_int(25)));
    }

    #[test]
    fn karma_weights_active_stake_within_the_active_pool() {
        // Equal amounts, karma 3:1 → still a 3:1 split — reputation matters
        // as much as capital for the active share (§11).
        let positions = [
            active(1, 100, Fx::from_int(3)),
            active(2, 100, Fx::from_int(1)),
        ];
        let shares = stake_yield_shares(&positions, Fx::from_int(100));
        assert_eq!(shares[0], (n(1), Fx::from_int(75)));
        assert_eq!(shares[1], (n(2), Fx::from_int(25)));
    }

    #[test]
    fn passive_stake_is_excluded_even_when_mixed_with_active() {
        // A large passive position sits alongside two equal active ones — the
        // passive stake must not dilute or claim any part of the pool.
        let positions = [passive(9, 10_000_000), active(1, 50, Fx::ONE), active(2, 50, Fx::ONE)];
        let shares = stake_yield_shares(&positions, Fx::from_int(100));
        assert_eq!(shares[0], (n(9), Fx::ZERO));
        assert_eq!(shares[1], (n(1), Fx::from_int(50)));
        assert_eq!(shares[2], (n(2), Fx::from_int(50)));
    }

    #[test]
    fn shares_conserve_the_pool() {
        let positions = [active(1, 30, Fx::ONE), active(2, 50, Fx::ONE), active(3, 20, Fx::ONE)];
        let pool = Fx::from_int(1000);
        let shares = stake_yield_shares(&positions, pool);
        let total = shares.iter().fold(Fx::ZERO, |acc, (_, s)| acc + *s);
        assert_eq!(total, pool);
    }
    #[test]
    fn shares_conserve_the_pool_across_a_sweep() {
        // Rounded fixed-point quotients do not sum exactly on their own
        // (a=1, b=1, c=30 misses by dust without the residual rule); every
        // combination here must conserve the pool exactly.
        for a in 1..=13u128 {
            for b in 1..=13u128 {
                for c in [1u128, 7, 30, 1_000_000] {
                    let positions = [
                        passive(9, 5_000),
                        active(1, a, Fx::ONE),
                        active(2, b, Fx::from_ratio(1, 3)),
                        active(3, c, Fx::ONE),
                    ];
                    let pool = Fx::from_int(1000);
                    let shares = stake_yield_shares(&positions, pool);
                    let total = shares.iter().fold(Fx::ZERO, |acc, (_, s)| acc + *s);
                    assert_eq!(total, pool, "a={a} b={b} c={c}");
                    assert!(shares.iter().all(|(_, s)| *s >= Fx::ZERO), "a={a} b={b} c={c}");
                    assert_eq!(shares[0].1, Fx::ZERO);
                }
            }
        }
    }

    #[test]
    fn splitting_active_stake_across_identities_is_reward_neutral() {
        // §15: stake-weighting makes identity-splitting reward-neutral. One
        // active position of 600 against a rival of 400 earns the same total
        // as the same 600 split into six identities of 100.
        let pool = Fx::from_int(1000);
        let whole = [active(1, 600, Fx::ONE), active(2, 400, Fx::ONE)];
        let whole_share = stake_yield_shares(&whole, pool)[0].1;
        let mut split: Vec<StakePosition> = (10..16).map(|i| active(i, 100, Fx::ONE)).collect();
        split.push(active(2, 400, Fx::ONE));
        let shares = stake_yield_shares(&split, pool);
        let split_total = shares[..6].iter().fold(Fx::ZERO, |acc, (_, s)| acc + *s);
        // Equal up to fixed-point dust: six rounded quotients against one,
        // each off by at most 2^-32 — bounded here at one millionth of a token.
        let gap = if split_total > whole_share { split_total - whole_share } else { whole_share - split_total };
        assert!(gap < Fx::from_ratio(1, 1_000_000), "gap={gap:?}");
        let gap600 = if whole_share > Fx::from_int(600) { whole_share - Fx::from_int(600) } else { Fx::from_int(600) - whole_share };
        assert!(gap600 < Fx::from_ratio(1, 1_000_000), "whole={whole_share:?}");
    }
}
