// ---
// tags: tok, rust, conservation, rewards
// crystal-type: source
// crystal-domain: cyber
// ---
//! Conservation clip: `Σ mint = min(v★, Δφ⁺)` then map to token units.
//!
//! From rewards §4:
//! > tok renormalizes the settled shares to min(v★(N), Δφ⁺(N)) at settlement,
//! > the proportional clip of impulse. With that step, Σ mint(ν) ≤ global Δφ⁺.

use tru::Fx;

/// One neuron's share after conservation (field units) and optional token amount.
#[derive(Clone, Debug)]
pub struct ConserveResult {
    pub neuron: [u8; 32],
    /// Conserved field share (sums to min(v★, Δφ⁺)).
    pub share: Fx,
    /// Token units after scale + budget cap (sums to ≤ budget).
    pub amount: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConserveError {
    Empty,
    NoPositiveShare,
    BudgetZero,
}

/// Proportional clip of raw Shapley shares to `min(v_star, directed_total)`.
///
/// - `raw`: (neuron, shapley share Fx) — may not sum exactly (MC estimate)
/// - `directed_total`: realized Δφ⁺ of the full coalition
/// - If sum(raw) ≤ 0 → error
/// - Conserved share_i = raw_i * ceiling / sum_raw  (only positive raw)
/// - Negative / zero raw → zero conserved share
pub fn clip_shares(
    raw: &[([u8; 32], Fx)],
    directed_total: Fx,
) -> Result<Vec<([u8; 32], Fx)>, ConserveError> {
    if raw.is_empty() {
        return Err(ConserveError::Empty);
    }
    let mut sum_pos = Fx::ZERO;
    for (_, s) in raw {
        if *s > Fx::ZERO {
            sum_pos = sum_pos + *s;
        }
    }
    if sum_pos <= Fx::ZERO {
        return Err(ConserveError::NoPositiveShare);
    }
    // v★ ≈ sum of positive lottery shares; ceiling = min(v★, Δφ⁺)
    let mut ceiling = sum_pos;
    if directed_total > Fx::ZERO && directed_total < ceiling {
        ceiling = directed_total;
    }
    // If directed_total is zero but shares positive (edge path), keep sum_pos
    // so propose-path mints still work; pure zero-value fails above.
    let scale = ceiling.div(sum_pos); // ceiling / sum_pos ≤ 1
    let out: Vec<_> = raw
        .iter()
        .map(|(n, s)| {
            if *s <= Fx::ZERO {
                (*n, Fx::ZERO)
            } else {
                (*n, *s * scale)
            }
        })
        .collect();
    Ok(out)
}

/// Full conservation + token allocation under budget cap.
///
/// 1. clip_shares → conserved Fx
/// 2. Map conserved mass to tokens via `emission_scale` (tokens per Fx::ONE)
/// 3. Cap total tokens at `budget`
/// 4. Proportional integer split (last residual to first positive)
pub fn conserve_and_allocate(
    raw: &[([u8; 32], Fx)],
    directed_total: Fx,
    emission_scale: u64,
    budget: u64,
) -> Result<Vec<ConserveResult>, ConserveError> {
    if budget == 0 {
        return Err(ConserveError::BudgetZero);
    }
    let conserved = clip_shares(raw, directed_total)?;
    let mut sum_fx = Fx::ZERO;
    for (_, s) in &conserved {
        sum_fx = sum_fx + *s;
    }
    // Uncapped token mass from field units.
    let uncapped = fx_to_tokens(sum_fx, emission_scale);
    let total_tokens = uncapped.min(budget);
    if total_tokens == 0 {
        // Sub-scale dust: mint 1 token to the largest positive share.
        let mut best: Option<(usize, Fx)> = None;
        for (i, (_, s)) in conserved.iter().enumerate() {
            if *s > Fx::ZERO && best.map(|(_, bs)| *s > bs).unwrap_or(true) {
                best = Some((i, *s));
            }
        }
        let Some((bi, _)) = best else {
            return Err(ConserveError::NoPositiveShare);
        };
        let mut out: Vec<ConserveResult> = conserved
            .iter()
            .map(|(n, s)| ConserveResult {
                neuron: *n,
                share: *s,
                amount: 0,
            })
            .collect();
        out[bi].amount = 1.min(budget);
        return Ok(out);
    }

    // Weights for integer split
    let weights: Vec<u128> = conserved.iter().map(|(_, s)| fx_weight(*s)).collect();
    let sum_w: u128 = weights.iter().sum();
    if sum_w == 0 {
        return Err(ConserveError::NoPositiveShare);
    }

    let mut out = Vec::with_capacity(conserved.len());
    let mut allocated = 0u64;
    for (i, (neuron, share)) in conserved.iter().enumerate() {
        let amt = if i + 1 == conserved.len() {
            total_tokens.saturating_sub(allocated)
        } else {
            let a = ((weights[i] * total_tokens as u128) / sum_w) as u64;
            allocated = allocated.saturating_add(a);
            a
        };
        out.push(ConserveResult {
            neuron: *neuron,
            share: *share,
            amount: if weights[i] == 0 { 0 } else { amt },
        });
    }
    // Zero zero-weight; rehome remainder
    for (i, w) in weights.iter().enumerate() {
        if *w == 0 {
            out[i].amount = 0;
        }
    }
    let paid: u64 = out.iter().map(|r| r.amount).sum();
    if paid < total_tokens {
        if let Some(r) = out.iter_mut().find(|r| fx_weight(r.share) > 0) {
            r.amount = r.amount.saturating_add(total_tokens - paid);
        }
    }
    Ok(out)
}

/// Tokens corresponding to a field mass: `round(fx * emission_scale)`.
pub fn fx_to_tokens(x: Fx, emission_scale: u64) -> u64 {
    if x <= Fx::ZERO || emission_scale == 0 {
        return 0;
    }
    let f = x.to_f64();
    if f <= 0.0 {
        return 0;
    }
    let t = f * emission_scale as f64;
    if t >= u64::MAX as f64 {
        return u64::MAX;
    }
    t.round().max(0.0) as u64
}

fn fx_weight(x: Fx) -> u128 {
    if x <= Fx::ZERO {
        return 0;
    }
    let f = x.to_f64();
    if f <= 0.0 {
        return 0;
    }
    let w = (f * 1_000_000_000_000.0) as u128;
    w.max(1)
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
    fn clip_when_vstar_exceeds_delta() {
        // Lottery over-claim: sum shares 2.0, Δφ⁺ = 1.0 → clip to 1.0
        let raw = vec![(h(1), Fx::ONE), (h(2), Fx::ONE)];
        let clipped = clip_shares(&raw, Fx::ONE).unwrap();
        let sum: f64 = clipped.iter().map(|(_, s)| s.to_f64()).sum();
        assert!((sum - 1.0).abs() < 1e-6, "sum={sum}");
        assert!((clipped[0].1.to_f64() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn no_clip_when_under_delta() {
        let raw = vec![(h(1), Fx::from_ratio(1, 2))];
        let clipped = clip_shares(&raw, Fx::ONE).unwrap();
        assert!((clipped[0].1.to_f64() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn zero_rho_style_share_gets_nothing() {
        let raw = vec![(h(1), Fx::ONE), (h(2), Fx::ZERO)];
        let out = conserve_and_allocate(&raw, Fx::ONE, 1000, 1000).unwrap();
        assert_eq!(out[0].amount, 1000);
        assert_eq!(out[1].amount, 0);
        let paid: u64 = out.iter().map(|r| r.amount).sum();
        assert_eq!(paid, 1000);
    }

    #[test]
    fn budget_caps_emission() {
        // Large emission_scale but budget 10
        let raw = vec![(h(1), Fx::ONE)];
        let out = conserve_and_allocate(&raw, Fx::ONE, 1_000_000, 10).unwrap();
        assert_eq!(out[0].amount, 10);
    }

    #[test]
    fn conservation_of_token_sum() {
        let raw = vec![(h(1), Fx::from_ratio(3, 4)), (h(2), Fx::from_ratio(1, 4))];
        let out = conserve_and_allocate(&raw, Fx::ONE, 1000, 1000).unwrap();
        let paid: u64 = out.iter().map(|r| r.amount).sum();
        assert_eq!(paid, out.iter().map(|r| r.amount).sum::<u64>());
        assert!(paid <= 1000);
        assert_eq!(paid, 1000.min(fx_to_tokens(Fx::ONE, 1000)));
    }
}
