// ---
// tags: tok, rust, plumb, rewards, pow
// crystal-type: source
// crystal-domain: cyber
// ---
//! The PoW-subsidy term of `specs/rewards.md` §11's reward equation:
//! `R_PoW · w_ν / Σ_μ w_μ`, where `w_ν` is a neuron's proven settlement work
//! this epoch (accepted settlement samples, §7–§8). Unlike the stake-yield
//! term, this split is karma- and stake-blind (§11, the "stakeless entry"
//! row): it opens a door into the reward that needs no stake and no prior
//! reputation, shared strictly in proportion to work done, so splitting one
//! signal into many identities earns no more than settling it once.

use tru::Fx;

/// Proportional split of a PoW-subsidy pool by proven settlement work.
///
/// - `pool`: this epoch's `R_PoW` budget.
/// - `work`: (neuron, `w_ν`) — accepted settlement samples; non-positive
///   entries earn nothing and do not count toward the total.
/// - If no neuron has positive work, every share is zero and the pool goes
///   unclaimed this epoch rather than panicking or dividing by zero.
pub fn pow_subsidy_shares(pool: Fx, work: &[([u8; 32], Fx)]) -> Vec<([u8; 32], Fx)> {
    let mut total = Fx::ZERO;
    for (_, w) in work {
        if *w > Fx::ZERO {
            total = total + *w;
        }
    }
    if total <= Fx::ZERO || pool <= Fx::ZERO {
        return work.iter().map(|(n, _)| (*n, Fx::ZERO)).collect();
    }
    work.iter()
        .map(|(n, w)| {
            if *w > Fx::ZERO {
                (*n, pool * (*w).div(total))
            } else {
                (*n, Fx::ZERO)
            }
        })
        .collect()
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
    fn splits_proportional_to_work_only() {
        let work = vec![(h(1), Fx::from_int(3)), (h(2), Fx::from_int(1))];
        let shares = pow_subsidy_shares(Fx::from_int(4), &work);
        assert!((shares[0].1.to_f64() - 3.0).abs() < 1e-6);
        assert!((shares[1].1.to_f64() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_work_earns_zero_regardless_of_stake_or_karma() {
        // No karma/stake input exists in this function's signature at all —
        // a neuron with w=0 draws nothing no matter how large its stake is.
        let work = vec![(h(1), Fx::from_int(5)), (h(2), Fx::ZERO)];
        let shares = pow_subsidy_shares(Fx::from_int(10), &work);
        assert!((shares[0].1.to_f64() - 10.0).abs() < 1e-6);
        assert_eq!(shares[1].1, Fx::ZERO);
    }

    #[test]
    fn no_positive_work_leaves_pool_unclaimed() {
        let work = vec![(h(1), Fx::ZERO), (h(2), Fx::ZERO)];
        let shares = pow_subsidy_shares(Fx::from_int(10), &work);
        assert!(shares.iter().all(|(_, s)| *s == Fx::ZERO));
    }

    #[test]
    fn conserves_the_pool_across_active_workers() {
        let work = vec![
            (h(1), Fx::from_int(2)),
            (h(2), Fx::from_int(5)),
            (h(3), Fx::from_int(3)),
        ];
        let pool = Fx::from_int(1000);
        let shares = pow_subsidy_shares(pool, &work);
        let sum: f64 = shares.iter().map(|(_, s)| s.to_f64()).sum();
        assert!((sum - 1000.0).abs() < 1e-3);
    }

    #[test]
    fn negative_work_is_treated_as_ineligible() {
        let work = vec![(h(1), Fx::ZERO - Fx::from_int(1)), (h(2), Fx::from_int(1))];
        let shares = pow_subsidy_shares(Fx::from_int(10), &work);
        assert_eq!(shares[0].1, Fx::ZERO);
        assert!((shares[1].1.to_f64() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn empty_work_list_returns_empty() {
        let shares = pow_subsidy_shares(Fx::from_int(10), &[]);
        assert!(shares.is_empty());
    }
}
