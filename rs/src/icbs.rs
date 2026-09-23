// ---
// tags: tok, rust, icbs, plumb
// crystal-type: source
// crystal-domain: cyber
// ---
//! ICBS market state — reserves and positions under conservation (row 25).
//!
//! Cost function per the truth market core (`cyber/launch.md` §"hybrid
//! economics"): `C(s_Y, s_N) = λ·√(s_Y² + s_N²)`, the self-scaling liquidity
//! that prices a position and doubles as its spam cost. This module holds
//! per-link market state (one `Position` per neuron) and the buy/sell
//! transitions that move it. Every trade settles through [`MintLedger`] as a
//! burn from the trader paired with a mint to the market's own reserve
//! account (or the reverse on sell), so the ledger's own conservation check
//! (`Σ balances = mint − burn`) holds before and after every trade without a
//! new ledger primitive — a market reserve is tokens in someone's custody,
//! not tokens destroyed or created.

use std::collections::BTreeMap;

use tru::Fx;

use crate::conservation::fx_to_tokens;
use crate::ledger::{LedgerError, MintLedger, NeuronId, TokenId};

/// One neuron's YES/NO exposure on one cyberlink's ICBS market.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub s_yes: Fx,
    pub s_no: Fx,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            s_yes: Fx::ZERO,
            s_no: Fx::ZERO,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MarketError {
    Ledger(LedgerError),
    NegativeShares,
    InsufficientPosition,
}

impl From<LedgerError> for MarketError {
    fn from(e: LedgerError) -> Self {
        MarketError::Ledger(e)
    }
}

/// ICBS cost: `C(s_Y, s_N) = λ·√(s_Y² + s_N²)` — the escrow a position's
/// current shares require, in field units.
pub fn cost(lambda: Fx, s_yes: Fx, s_no: Fx) -> Fx {
    let sum_sq = s_yes * s_yes + s_no * s_no;
    lambda * sum_sq.sqrt()
}

/// A cyberlink's ICBS market: positions escrowed against a reserve account
/// under `escrow_token`, both held in the same [`MintLedger`] the mint path
/// already conserves.
pub struct Market {
    pub link: TokenId,
    pub escrow_token: TokenId,
    pub reserve_neuron: NeuronId,
    pub lambda: Fx,
    positions: BTreeMap<NeuronId, Position>,
}

impl Market {
    pub fn new(link: TokenId, escrow_token: TokenId, reserve_neuron: NeuronId, lambda: Fx) -> Self {
        Self {
            link,
            escrow_token,
            reserve_neuron,
            lambda,
            positions: BTreeMap::new(),
        }
    }

    pub fn position(&self, neuron: &NeuronId) -> Position {
        self.positions.get(neuron).copied().unwrap_or_default()
    }

    /// The reserve is a ledger balance, not shadow state — it is always
    /// exactly what the trades moved into `reserve_neuron`.
    pub fn reserve(&self, ledger: &MintLedger) -> u64 {
        ledger.balance(&self.reserve_neuron, &self.escrow_token)
    }

    /// Add `d_yes`/`d_no` shares (either may be zero, neither negative) to
    /// `neuron`'s position. Cost is non-decreasing as shares grow from a
    /// non-negative position, so the price is always ≥ 0: burn it from the
    /// trader's escrow-token balance and mint the same amount to the
    /// market's reserve. Returns the price paid, in token units.
    pub fn buy(
        &mut self,
        ledger: &mut MintLedger,
        neuron: NeuronId,
        d_yes: Fx,
        d_no: Fx,
        emission_scale: u64,
    ) -> Result<u64, MarketError> {
        if d_yes < Fx::ZERO || d_no < Fx::ZERO {
            return Err(MarketError::NegativeShares);
        }
        let pos = self.position(&neuron);
        let before = cost(self.lambda, pos.s_yes, pos.s_no);
        let after_yes = pos.s_yes + d_yes;
        let after_no = pos.s_no + d_no;
        let after = cost(self.lambda, after_yes, after_no);
        let price = fx_to_tokens(after - before, emission_scale);

        if price > 0 {
            ledger.burn(neuron, self.escrow_token, price)?;
            ledger.mint_batch(self.escrow_token, &[(self.reserve_neuron, price)])?;
        }
        self.positions.insert(
            neuron,
            Position {
                s_yes: after_yes,
                s_no: after_no,
            },
        );
        Ok(price)
    }

    /// Remove `d_yes`/`d_no` shares (neither negative, neither exceeding the
    /// held position) from `neuron`'s position, refunding the cost delta from
    /// the market's reserve back to the trader. Returns the refund paid.
    pub fn sell(
        &mut self,
        ledger: &mut MintLedger,
        neuron: NeuronId,
        d_yes: Fx,
        d_no: Fx,
        emission_scale: u64,
    ) -> Result<u64, MarketError> {
        if d_yes < Fx::ZERO || d_no < Fx::ZERO {
            return Err(MarketError::NegativeShares);
        }
        let pos = self.position(&neuron);
        if d_yes > pos.s_yes || d_no > pos.s_no {
            return Err(MarketError::InsufficientPosition);
        }
        let before = cost(self.lambda, pos.s_yes, pos.s_no);
        let after_yes = pos.s_yes - d_yes;
        let after_no = pos.s_no - d_no;
        let after = cost(self.lambda, after_yes, after_no);
        let refund = fx_to_tokens(before - after, emission_scale);

        if refund > 0 {
            ledger.burn(self.reserve_neuron, self.escrow_token, refund)?;
            ledger.mint_batch(self.escrow_token, &[(neuron, refund)])?;
        }
        self.positions.insert(
            neuron,
            Position {
                s_yes: after_yes,
                s_no: after_no,
            },
        );
        Ok(refund)
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

    fn market() -> (Market, MintLedger) {
        let m = Market::new(h(0xA), h(0xE), h(0xF), Fx::ONE);
        (m, MintLedger::new())
    }

    #[test]
    fn cost_matches_pythagorean_triple() {
        // 3-4-5: λ=1, s_Y=3, s_N=4 → cost = 5 exactly.
        let c = cost(Fx::ONE, Fx::from_int(3), Fx::from_int(4));
        assert!((c.to_f64() - 5.0).abs() < 1e-9, "cost={}", c.to_f64());
    }

    #[test]
    fn cost_is_zero_at_origin() {
        assert_eq!(cost(Fx::ONE, Fx::ZERO, Fx::ZERO), Fx::ZERO);
    }

    #[test]
    fn buy_debits_trader_and_credits_reserve_by_the_same_amount() {
        let (mut mkt, mut ledger) = market();
        ledger.mint_batch(mkt.escrow_token, &[(h(1), 1_000)]).unwrap();

        let price = mkt.buy(&mut ledger, h(1), Fx::from_int(3), Fx::from_int(4), 1).unwrap();

        assert!(price > 0);
        assert_eq!(ledger.balance(&h(1), &mkt.escrow_token), 1_000 - price);
        assert_eq!(mkt.reserve(&ledger), price);
        assert!(ledger.check_token(mkt.escrow_token));
    }

    #[test]
    fn sell_refunds_from_reserve_and_shrinks_position() {
        let (mut mkt, mut ledger) = market();
        ledger.mint_batch(mkt.escrow_token, &[(h(1), 1_000)]).unwrap();
        mkt.buy(&mut ledger, h(1), Fx::from_int(3), Fx::from_int(4), 1).unwrap();

        let refund = mkt
            .sell(&mut ledger, h(1), Fx::from_int(3), Fx::from_int(4), 1)
            .unwrap();

        assert_eq!(mkt.position(&h(1)), Position::default());
        assert_eq!(mkt.reserve(&ledger), 0);
        assert_eq!(refund, 5); // round trip on an exact Pythagorean triple is lossless
        assert_eq!(ledger.balance(&h(1), &mkt.escrow_token), 1_000);
        assert!(ledger.check_token(mkt.escrow_token));
    }

    #[test]
    fn partial_sell_leaves_a_smaller_position_and_a_smaller_refund() {
        let (mut mkt, mut ledger) = market();
        ledger.mint_batch(mkt.escrow_token, &[(h(1), 1_000)]).unwrap();
        mkt.buy(&mut ledger, h(1), Fx::from_int(6), Fx::from_int(8), 1).unwrap(); // cost 10

        let refund = mkt.sell(&mut ledger, h(1), Fx::from_int(3), Fx::from_int(4), 1).unwrap();

        // cost(3,4) = 5, so selling half the shares (which is not half the cost,
        // since cost is not linear) refunds cost(6,8) - cost(3,4) = 10 - 5 = 5.
        assert_eq!(refund, 5);
        assert_eq!(mkt.position(&h(1)), Position { s_yes: Fx::from_int(3), s_no: Fx::from_int(4) });
        assert!(ledger.check_token(mkt.escrow_token));
    }

    #[test]
    fn buy_rejects_negative_shares() {
        let (mut mkt, mut ledger) = market();
        assert_eq!(
            mkt.buy(&mut ledger, h(1), Fx::from_int(-1), Fx::ZERO, 1),
            Err(MarketError::NegativeShares)
        );
    }

    #[test]
    fn sell_rejects_more_than_the_held_position() {
        let (mut mkt, mut ledger) = market();
        ledger.mint_batch(mkt.escrow_token, &[(h(1), 1_000)]).unwrap();
        mkt.buy(&mut ledger, h(1), Fx::from_int(1), Fx::ZERO, 1).unwrap();

        assert_eq!(
            mkt.sell(&mut ledger, h(1), Fx::from_int(2), Fx::ZERO, 1),
            Err(MarketError::InsufficientPosition)
        );
    }

    #[test]
    fn two_traders_share_one_market_and_stay_conserved() {
        let (mut mkt, mut ledger) = market();
        ledger.mint_batch(mkt.escrow_token, &[(h(1), 1_000), (h(2), 1_000)]).unwrap();

        mkt.buy(&mut ledger, h(1), Fx::from_int(3), Fx::from_int(4), 1).unwrap();
        mkt.buy(&mut ledger, h(2), Fx::from_int(6), Fx::from_int(8), 1).unwrap();

        assert!(ledger.check_token(mkt.escrow_token));
        assert_eq!(
            mkt.reserve(&ledger),
            ledger.total_minted(&mkt.escrow_token) - 2_000
        );
    }
}
