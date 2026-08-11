// ---
// tags: tok, rust, mint, plumb
// crystal-type: source
// crystal-domain: cyber
// ---
//! Execute settle → PLUMB mint legs under conservation.

use cyber_hemera::hash as hemera_hash;
use tru::Fx;

use crate::conservation::{conserve_and_allocate, ConserveError};
use crate::ledger::{LedgerError, MintLedger, NeuronId, TokenId};

/// One PLUMB mint leg (create Coin units to a holder Card/neuron).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MintLeg {
    pub to: NeuronId,
    pub token: TokenId,
    pub amount: u64,
}

/// Result of executing a conserved settle mint.
#[derive(Clone, Debug)]
pub struct MintReceipt {
    /// Hemera over (reason ‖ sorted legs).
    pub intent_hash: [u8; 32],
    pub legs: Vec<MintLeg>,
    /// Total minted this intent.
    pub total: u64,
    /// Conserved field mass (min(v★, Δφ⁺)).
    pub conserved_fx: Fx,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MintError {
    Conserve(ConserveError),
    Ledger(LedgerError),
    EmptyLegs,
}

impl From<ConserveError> for MintError {
    fn from(e: ConserveError) -> Self {
        MintError::Conserve(e)
    }
}

impl From<LedgerError> for MintError {
    fn from(e: LedgerError) -> Self {
        MintError::Ledger(e)
    }
}

/// Apply conservation clip then mint into ledger.
///
/// `raw_shares` — Shapley lottery shares
/// `directed_total` — Δφ⁺ ceiling
/// `emission_scale` — tokens per Fx::ONE of conserved mass
/// `budget` — hard cap on this epoch's emission
/// `reason` — settle receipt hash / content id binding the mint
pub fn execute_settle_mints(
    ledger: &mut MintLedger,
    token: TokenId,
    raw_shares: &[([u8; 32], Fx)],
    directed_total: Fx,
    emission_scale: u64,
    budget: u64,
    reason: &[u8; 32],
) -> Result<MintReceipt, MintError> {
    let conserved = conserve_and_allocate(raw_shares, directed_total, emission_scale, budget)?;
    let legs: Vec<MintLeg> = conserved
        .iter()
        .filter(|r| r.amount > 0)
        .map(|r| MintLeg {
            to: r.neuron,
            token,
            amount: r.amount,
        })
        .collect();
    if legs.is_empty() {
        return Err(MintError::EmptyLegs);
    }
    let batch: Vec<(NeuronId, u64)> = legs.iter().map(|l| (l.to, l.amount)).collect();
    let total = ledger.mint_batch(token, &batch)?;
    let conserved_fx = conserved.iter().fold(Fx::ZERO, |acc, r| acc + r.share);
    Ok(MintReceipt {
        intent_hash: intent_hash(reason, &legs),
        legs,
        total,
        conserved_fx,
    })
}

fn intent_hash(reason: &[u8; 32], legs: &[MintLeg]) -> [u8; 32] {
    let mut sorted = legs.to_vec();
    sorted.sort_by(|a, b| a.to.cmp(&b.to).then(a.amount.cmp(&b.amount)));
    let mut buf = Vec::with_capacity(32 + sorted.len() * 40);
    buf.extend_from_slice(b"tok-mint-v0");
    buf.extend_from_slice(reason);
    for l in &sorted {
        buf.extend_from_slice(&l.to);
        buf.extend_from_slice(&l.token);
        buf.extend_from_slice(&l.amount.to_le_bytes());
    }
    *hemera_hash(&buf)
        .as_bytes()
        .first_chunk::<32>()
        .unwrap_or(&[0u8; 32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tru::Fx;

    fn h(b: u8) -> [u8; 32] {
        let mut x = [0u8; 32];
        x[0] = b;
        x
    }

    #[test]
    fn execute_mints_under_clip() {
        let mut led = MintLedger::new();
        let token = h(7);
        // Over-claim lottery vs Δφ⁺
        let raw = vec![(h(1), Fx::ONE), (h(2), Fx::ONE)];
        let rec = execute_settle_mints(
            &mut led,
            token,
            &raw,
            Fx::ONE, // directed = 1, v★≈2 → clip
            1000,
            1000,
            &h(0xAA),
        )
        .unwrap();
        assert!(led.check_token(token));
        assert_eq!(rec.total, led.supply(&token));
        // conserved mass ≈ 1.0 → 1000 tokens split
        assert_eq!(rec.total, 1000);
        assert_eq!(rec.legs.len(), 2);
    }
}
