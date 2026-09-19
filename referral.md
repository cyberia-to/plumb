---
tags: cyber, plumb, soft3, token, oikos, referral
alias: referral, referral birth allocation, sybil-resistant referral
crystal-type: spec
crystal-domain: cyber
---

# referral — birth allocation

[[cyber/launch|launch]] core 5 rides the referral on [[oikos]] book
registration. registration is a [[cyberlink]]: the referrer's [[neuron]]
links the newcomer's name. at the birth of the newcomer's home book, a
fixed share `r` of the book's birth mint goes to the referrer; the rest
mints home to the newcomer.

this page specs the tok-owned half of property 32: the birth allocation.
fee-sharing pro rata to $ν holders over the book's life, and the
transferability of $ν itself, are separate, still-open clauses (launch.md
decisions log).

## the rule

1. a birth mint of `total` tokens splits `r · total` to the referrer and
   `(1 − r) · total` to the newcomer, both legs committed atomically under
   [[plumb]] token conservation.
2. no referrer — a book rooted without a referral cyberlink — sends the
   whole birth mint home.
3. a neuron can never be its own referrer; self-referral is rejected
   before any mint leg is built.
4. `r` is a genesis parameter, not a constant in this crate. its exact
   value is still an open decision (launch.md decisions log: "the referral
   share r"); this crate accepts it as an argument and enforces the split
   invariant for whatever value governance settles on.

Sybil resistance (property 33 — referring an inactive or fake account
pays nothing) follows once book activity determines whether `total` is
ever nonzero, not from this allocation rule; it is a separate clause.

## implementation

`tok::birth_mint` in `rs/src/referral.rs`:

| input | meaning |
|---|---|
| `token` | the book's token id |
| `neuron` | the newcomer rooting the book |
| `referrer` | the neuron whose cyberlink registered the name, if any |
| `total` | the birth mint amount |
| `r` | the referral share, `Fx` in `[0, 1]` |

returns a `BirthMintReceipt { neuron_amount, referrer_amount }` with
`neuron_amount + referrer_amount == total`, or `ReferralError::SelfReferral`
before touching the ledger.

## see also

- [[oikos-book|oikos book]] — the home book this allocation funds at birth
- [[oikos]] — the general proposal
- [[cyber/launch|launch]] — core 5, property 32
- [[plumb]] — the mint operation and its conservation law

---

discover all [[concepts]]
