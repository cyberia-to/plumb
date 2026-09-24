// ---
// tags: tok, rust, plumb, rewards, service-fees
// crystal-type: source
// crystal-domain: cyber
// ---
//! The service-fee term of `tru/specs/rewards.md` §11's reward equation:
//! `Σ_{q ∈ Q_ν} γ(1−β) fee_q`. A neuron answering a query is paid directly
//! by the asker: γ is the servicer's share of the fee, β the fraction §8
//! burns, so `γ(1−β)` of each `fee_q` reaches the servicer and the rest is
//! burned or routed to the security budget (§8), neither of which this
//! function computes. Unlike the mint or stake-yield terms this pays per
//! query answered, not per epoch pool split — there is nothing to
//! renormalize, only to sum per servicer.

use std::collections::BTreeMap;

use neuron_id::NeuronId;
use tru::Fx;

/// One query's fee, credited to the neuron that served it.
#[derive(Clone, Copy, Debug)]
pub struct ServedQuery {
    pub servicer: NeuronId,
    pub fee: Fx,
}

/// Sum `γ(1−β)·fee_q` over every query a neuron served, aggregated per
/// servicer. `gamma` and `beta` are clipped to `[0,1]` first — a fee split
/// only ever moves value out of `fee_q`, never manufactures more of it, and
/// a burn fraction can neither go negative nor exceed the whole fee.
pub fn service_fee_shares(served: &[ServedQuery], gamma: Fx, beta: Fx) -> Vec<(NeuronId, Fx)> {
    let rate = clip_unit(gamma) * (Fx::ONE - clip_unit(beta));

    let mut totals: BTreeMap<NeuronId, Fx> = BTreeMap::new();
    let mut order: Vec<NeuronId> = Vec::new();
    for q in served {
        let share = rate * q.fee;
        totals
            .entry(q.servicer)
            .and_modify(|s| *s = *s + share)
            .or_insert_with(|| {
                order.push(q.servicer);
                share
            });
    }
    order.into_iter().map(|n| (n, totals[&n])).collect()
}

fn clip_unit(x: Fx) -> Fx {
    if x < Fx::ZERO {
        Fx::ZERO
    } else if x > Fx::ONE {
        Fx::ONE
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(b: u8) -> NeuronId {
        let mut x = [0u8; 32];
        x[0] = b;
        x
    }

    fn q(servicer: NeuronId, fee: i64) -> ServedQuery {
        ServedQuery {
            servicer,
            fee: Fx::from_int(fee),
        }
    }

    #[test]
    fn single_query_pays_gamma_times_one_minus_beta() {
        // gamma = 0.5, beta = 0.2 -> rate = 0.4; fee = 10 -> share = 4,
        // up to the rounding two chained fixed-point multiplies carry.
        let gamma = Fx::from_ratio(1, 2);
        let beta = Fx::from_ratio(1, 5);
        let shares = service_fee_shares(&[q(n(1), 10)], gamma, beta);
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].0, n(1));
        assert!((shares[0].1.to_f64() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn multiple_queries_from_one_servicer_sum() {
        let gamma = Fx::ONE;
        let beta = Fx::ZERO;
        let shares = service_fee_shares(&[q(n(1), 3), q(n(1), 5)], gamma, beta);
        assert_eq!(shares, vec![(n(1), Fx::from_int(8))]);
    }

    #[test]
    fn distinct_servicers_stay_separate() {
        let gamma = Fx::ONE;
        let beta = Fx::ZERO;
        let shares = service_fee_shares(&[q(n(1), 3), q(n(2), 5)], gamma, beta);
        assert_eq!(shares, vec![(n(1), Fx::from_int(3)), (n(2), Fx::from_int(5))]);
    }

    #[test]
    fn gamma_zero_pays_nothing() {
        let shares = service_fee_shares(&[q(n(1), 10)], Fx::ZERO, Fx::ZERO);
        assert_eq!(shares, vec![(n(1), Fx::ZERO)]);
    }

    #[test]
    fn beta_one_burns_the_whole_fee() {
        let shares = service_fee_shares(&[q(n(1), 10)], Fx::ONE, Fx::ONE);
        assert_eq!(shares, vec![(n(1), Fx::ZERO)]);
    }

    #[test]
    fn beta_above_one_clips_to_zero_payout_not_negative() {
        let shares = service_fee_shares(&[q(n(1), 10)], Fx::ONE, Fx::from_int(2));
        assert_eq!(shares, vec![(n(1), Fx::ZERO)]);
    }

    #[test]
    fn gamma_above_one_clips_to_one() {
        let a = service_fee_shares(&[q(n(1), 10)], Fx::from_int(2), Fx::ZERO);
        let b = service_fee_shares(&[q(n(1), 10)], Fx::ONE, Fx::ZERO);
        assert_eq!(a, b);
    }

    #[test]
    fn negative_beta_clips_to_zero_burn() {
        let a = service_fee_shares(&[q(n(1), 10)], Fx::ONE, Fx::ZERO - Fx::from_int(1));
        let b = service_fee_shares(&[q(n(1), 10)], Fx::ONE, Fx::ZERO);
        assert_eq!(a, b);
    }

    #[test]
    fn empty_served_list_returns_empty() {
        assert!(service_fee_shares(&[], Fx::ONE, Fx::ZERO).is_empty());
    }

    #[test]
    fn sum_of_shares_never_exceeds_sum_of_fees() {
        let gamma = Fx::from_ratio(7, 10);
        let beta = Fx::from_ratio(3, 10);
        let served = [q(n(1), 4), q(n(2), 9), q(n(1), 2)];
        let total_fee: f64 = served.iter().map(|q| q.fee.to_f64()).sum();
        let shares = service_fee_shares(&served, gamma, beta);
        let total_share: f64 = shares.iter().map(|(_, s)| s.to_f64()).sum();
        assert!(total_share <= total_fee + 1e-9);
    }
}
