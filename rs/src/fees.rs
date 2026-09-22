// ---
// tags: tok, rust, referral, plumb, oikos, fees
// crystal-type: source
// crystal-domain: cyber
// ---
//! Fee distribution of a home book — the second clause of property 32 and
//! the whole of property 33 (`cyber/launch.md`, core 5).
//!
//! > holding $ν is the right to a pro-rata share of the fees ν's book
//! > collects for as long as the book lives. the referrer is paid in the
//! > referee's own token, from the referee's own economy: a fake account
//! > has a book with no fees and a token worth nothing, so referring it
//! > pays nothing.
//!
//! A book's fees accumulate, in the book's own token, in its fee pool — the
//! account [`fee_pool`] derives from the token id. [`distribute_fees`] pays
//! the pool out to every holder of the token pro rata by balance, by
//! `MintLedger::transfer`, so supply is untouched and the ledger's
//! conservation check runs on every leg. Nothing is minted here.
//!
//! Property 33 is then a consequence, not a rule: the referrer's income from
//! a book is `r · fees` when it holds `r` of $ν (its birth share, `referral.rs`);
//! a book with no fees pays nothing however many such books are referred, and
//! the payout is a function of fees collected, never of heads referred.

use cyber_hemera::hash as hemera_hash;

use crate::ledger::{LedgerError, MintLedger, NeuronId, TokenId};

/// Receipt of one distribution: what left the pool and who received it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeReceipt {
    /// Fees paid out this call; equals the sum of `payouts`.
    pub distributed: u64,
    /// (holder, amount) for every holder that received a nonzero amount,
    /// ascending by neuron id.
    pub payouts: Vec<(NeuronId, u64)>,
}

/// The account a book's fees accumulate in: `hemera("oikos-fees-v0" ‖ token)`.
/// A pure function of the token id, no state read.
pub fn fee_pool(token: &TokenId) -> NeuronId {
    let mut buf = Vec::with_capacity(13 + 32);
    buf.extend_from_slice(b"oikos-fees-v0");
    buf.extend_from_slice(token);
    *hemera_hash(&buf)
        .as_bytes()
        .first_chunk::<32>()
        .unwrap_or(&[0u8; 32])
}

/// Pay the whole balance of `fee_pool(token)` to the token's holders pro rata
/// by balance (the pool itself excluded). Integer split by largest remainder:
/// `floor(fees · b_i / Σb)` each, the leftover units one apiece to the largest
/// fractional parts, so `Σ payouts == fees` exactly. Every leg is a transfer
/// under `check_token`; on any failure the ledger is restored and the error
/// returned. An empty pool or no holders distributes nothing.
pub fn distribute_fees(ledger: &mut MintLedger, token: TokenId) -> Result<FeeReceipt, LedgerError> {
    let pool = fee_pool(&token);
    let fees = ledger.balance(&pool, &token);
    let holders: Vec<(NeuronId, u64)> = ledger
        .holders(&token)
        .into_iter()
        .filter(|(n, _)| *n != pool)
        .collect();
    let total: u128 = holders.iter().map(|(_, b)| *b as u128).sum();
    if fees == 0 || total == 0 {
        return Ok(FeeReceipt { distributed: 0, payouts: vec![] });
    }

    // (holder, floor share, remainder numerator)
    let mut split: Vec<(NeuronId, u64, u128)> = holders
        .iter()
        .map(|(n, b)| {
            let num = fees as u128 * *b as u128;
            (*n, (num / total) as u64, num % total)
        })
        .collect();
    let floored: u64 = split.iter().map(|(_, p, _)| *p).sum();
    let mut leftover = fees - floored; // < holders.len(), each remainder < total
    let mut by_remainder: Vec<usize> = (0..split.len()).collect();
    by_remainder.sort_by(|&i, &j| split[j].2.cmp(&split[i].2).then(split[i].0.cmp(&split[j].0)));
    for i in by_remainder {
        if leftover == 0 {
            break;
        }
        split[i].1 += 1;
        leftover -= 1;
    }

    let snapshot = ledger.clone();
    let mut payouts = Vec::with_capacity(split.len());
    for (holder, amount, _) in split {
        if amount == 0 {
            continue;
        }
        if let Err(e) = ledger.transfer(pool, holder, token, amount) {
            *ledger = snapshot;
            return Err(e);
        }
        payouts.push((holder, amount));
    }
    let distributed = payouts.iter().map(|(_, a)| *a).sum();
    Ok(FeeReceipt { distributed, payouts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::BookRegistry;
    use crate::referral::birth_mint;
    use tru::Fx;

    fn n(b: u8) -> NeuronId {
        let mut x = [0u8; 32];
        x[0] = b;
        x[1] = 0xEE;
        x
    }
    fn n2(i: u32) -> NeuronId {
        let mut x = [0u8; 32];
        x[..4].copy_from_slice(&i.to_le_bytes());
        x[4] = 0xFA;
        x
    }
    fn r() -> Fx {
        Fx::from_ratio(1, 10)
    }

    /// Root `newcomer`'s book with `referrer` linking the name; birth mint 1000.
    fn refer(ledger: &mut MintLedger, reg: &mut BookRegistry, referrer: NeuronId, newcomer: NeuronId) -> TokenId {
        let token = reg.root(newcomer).unwrap();
        birth_mint(ledger, token, newcomer, Some(referrer), 1000, r()).unwrap();
        token
    }

    /// A client acquires `amount` of the book's token and pays it as fees.
    fn pay_fees(ledger: &mut MintLedger, token: TokenId, client: NeuronId, amount: u64) {
        ledger.mint_batch(token, &[(client, amount)]).unwrap();
        ledger.transfer(client, fee_pool(&token), token, amount).unwrap();
    }

    #[test]
    fn inactive_book_pays_the_referrer_nothing() {
        let (mut led, mut reg) = (MintLedger::new(), BookRegistry::new());
        let token = refer(&mut led, &mut reg, n(1), n(2));
        assert_eq!(led.balance(&n(1), &token), 100); // birth share held, worth its fees: none
        let rec = distribute_fees(&mut led, token).unwrap();
        assert_eq!(rec, FeeReceipt { distributed: 0, payouts: vec![] });
        assert_eq!(led.balance(&n(1), &token), 100);
        assert!(led.check_token(token));
    }

    #[test]
    fn sybil_swarm_of_fake_books_yields_zero_fee_income() {
        // One referrer links 500 fake names. Every book is rooted and birth-
        // minted, so the referrer holds r of 500 tokens — and collects nothing,
        // because none of the 500 books ever earns a fee.
        let (mut led, mut reg) = (MintLedger::new(), BookRegistry::new());
        let referrer = n(1);
        let tokens: Vec<TokenId> = (0..500u32).map(|i| refer(&mut led, &mut reg, referrer, n2(i))).collect();
        let mut income = 0u64;
        for token in &tokens {
            let before = led.balance(&referrer, token);
            let rec = distribute_fees(&mut led, *token).unwrap();
            income += rec.distributed;
            assert_eq!(led.balance(&referrer, token), before);
            assert!(led.check_token(*token));
        }
        assert_eq!(income, 0);
    }

    #[test]
    fn genuine_book_pays_holders_pro_rata_under_conservation() {
        let (mut led, mut reg) = (MintLedger::new(), BookRegistry::new());
        let token = refer(&mut led, &mut reg, n(1), n(2)); // referrer 100, newcomer 900
        pay_fees(&mut led, token, n(3), 500);
        let supply = led.supply(&token);
        let rec = distribute_fees(&mut led, token).unwrap();
        assert_eq!(rec.distributed, 500);
        assert_eq!(led.balance(&n(1), &token), 100 + 50); // r · fees
        assert_eq!(led.balance(&n(2), &token), 900 + 450);
        assert_eq!(led.balance(&fee_pool(&token), &token), 0);
        assert_eq!(led.supply(&token), supply);
        assert!(led.check_token(token));
    }

    #[test]
    fn payout_scales_with_fees_collected_not_heads_referred() {
        // The referrer's income from 200 fake books plus one genuine book equals
        // its income from the genuine book alone.
        let income = |fakes: u32| {
            let (mut led, mut reg) = (MintLedger::new(), BookRegistry::new());
            let referrer = n(1);
            for i in 0..fakes {
                refer(&mut led, &mut reg, referrer, n2(i));
            }
            let real = refer(&mut led, &mut reg, referrer, n(7));
            pay_fees(&mut led, real, n(8), 12_340);
            let mut got = 0u64;
            for (_, t) in (0..fakes).map(|i| (i, BookRegistry::book_token(&n2(i)))).chain([(0, real)]) {
                let before = led.balance(&referrer, &t);
                distribute_fees(&mut led, t).unwrap();
                got += led.balance(&referrer, &t) - before;
            }
            got
        };
        let alone = income(0);
        assert_eq!(alone, 1_234); // r · fees at r = 1/10, holders 100:900
        assert_eq!(income(200), alone);
    }

    #[test]
    fn distribution_is_exact_across_a_sweep() {
        // Random holder balances and fee sizes: Σ payouts == fees, the pool
        // empties, each payout is within one unit of floor(fees · b / Σb), and
        // the ledger's own conservation check holds after every distribution.
        let token = [0xCCu8; 32];
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = || {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            x >> 33
        };
        for _ in 0..300 {
            let mut led = MintLedger::new();
            let k = 1 + (next() % 9) as u32;
            let holders: Vec<(NeuronId, u64)> = (0..k).map(|i| (n2(i), 1 + next() % 100_000)).collect();
            led.mint_batch(token, &holders).unwrap();
            let fees = next() % 1_000_000;
            pay_fees(&mut led, token, n(0x55), fees);
            let total: u128 = holders.iter().map(|(_, b)| *b as u128).sum();
            let rec = distribute_fees(&mut led, token).unwrap();
            assert_eq!(rec.distributed, fees);
            assert_eq!(rec.payouts.iter().map(|(_, a)| *a).sum::<u64>(), fees);
            assert_eq!(led.balance(&fee_pool(&token), &token), 0);
            assert!(led.check_token(token));
            for (h, b) in &holders {
                let got = led.balance(h, &token) - b;
                let floor = (fees as u128 * *b as u128 / total) as u64;
                assert!(got == floor || got == floor + 1, "got={got} floor={floor}");
            }
        }
    }

    #[test]
    fn a_pool_with_no_holders_keeps_its_fees() {
        let token = [0xDDu8; 32];
        let mut led = MintLedger::new();
        pay_fees(&mut led, token, n(9), 77);
        // the client spent its whole balance; only the pool holds the token
        let rec = distribute_fees(&mut led, token).unwrap();
        assert_eq!(rec.distributed, 0);
        assert_eq!(led.balance(&fee_pool(&token), &token), 77);
        assert!(led.check_token(token));
    }

    #[test]
    fn fee_pool_is_deterministic_and_distinct_per_token() {
        assert_eq!(fee_pool(&[1u8; 32]), fee_pool(&[1u8; 32]));
        assert_ne!(fee_pool(&[1u8; 32]), fee_pool(&[2u8; 32]));
        assert_ne!(fee_pool(&[1u8; 32]), BookRegistry::book_token(&[1u8; 32]));
    }
}
