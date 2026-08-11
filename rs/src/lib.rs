// ---
// tags: tok, rust, plumb, conservation
// crystal-type: source
// crystal-domain: cyber
// ---
//! tok — value layer: conservation clip + settle mint (PLUMB mint path).
//!
//! Spec: `tru/specs/rewards.md` §4 — tok renormalizes settled shares to
//! `min(v★(N), Δφ⁺(N))` so `Σ mint(ν) ≤` realized directed impulse.
//!
//! PLUMB laws (this crate enforces token conservation for settle mints):
//! 1. Token conservation — Σ balances = mints − burns per Coin class
//! 2. Atomicity — all mint legs from one receipt commit or none do
//!
//! Pay/lock/update/burn Intent composition stays for a fuller PLUMB runtime;
//! this module closes the settle→mint conservation hole.

pub mod conservation;
pub mod ledger;
pub mod mint;

pub use conservation::{clip_shares, conserve_and_allocate, ConserveError, ConserveResult};
pub use ledger::{LedgerError, MintLedger, TokenId};
pub use mint::{execute_settle_mints, MintError, MintLeg, MintReceipt};
