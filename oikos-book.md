---
tags: cyber, plumb, soft3, token, oikos
alias: oikos book, home book, book registry, personal chain token
crystal-type: spec
crystal-domain: cyber
---

# oikos book — the home-book root, phase 1 scope

[[oikos]] proposes one home book for one token, governed by an issuer's
authenticated neuron. [[cyber/launch|launch]] core 5 scopes phase 1 to the
narrow case: every [[neuron]] roots exactly one home book, with its own
token, on the same binary that runs bostrom and pussy. Registration then
links that book's name and state root into the [[cybergraph]] — the second
half of property 30, outside this crate.

this page specs the tok-owned half: the ledger primitive that lets a
neuron root its book and derives the book's token id.

## the rule

1. a neuron's book token id is `hemera("oikos-book-v0" ‖ neuron)`, a pure
   function of the neuron id. two neurons never collide on a book; the
   same neuron always finds the same book without asking a registry.
2. rooting is the one-time act of registering that a neuron has claimed
   its book. a neuron roots its home book once; a second `root` for the
   same neuron fails. this is the phase 1 restriction of the general
   oikos rule ("one issuer may govern several books") to exactly one.
3. rooting is a ledger-local invariant. it does not itself mint tokens,
   settle balances, or register the book's name into the cybergraph —
   those are separate operations layered on top (birth mint under
   [[plumb]] conservation; registration as a naming cyberlink, tracked by
   property 30's cybergraph half).

## implementation

`tok::BookRegistry` in `rs/src/ledger.rs`:

| method | does |
|---|---|
| `book_token(&neuron)` | the deterministic token id, no state read |
| `root(neuron)` | claims the neuron's one home book, fails on a second call |
| `book_of(&neuron)` | the rooted token id, if any |
| `is_rooted(&neuron)` | whether the neuron has rooted its book |

conservation of the book token itself — mint, burn, transfer — is the
existing `MintLedger` applied to `BookRegistry::book_token(neuron)` as the
`TokenId`; `BookRegistry` only owns the one-neuron-one-book invariant.

## what this does not close

- registration as a naming cyberlink into the cybergraph (property 30's
  other half, owned by cybergraph).
- the birth mint of $ν to the rooting neuron and the referral share
  (property 32).
- domain finality for the book (property 31, owned by foculus).

## see also

- [[oikos]] — the general proposal this narrows
- [[cyber/launch|launch]] — core 5, property 30
- [[plumb]] — the five operations the book's balances move through
- [[cybergraph]] — where the book's name and state root register

---

discover all [[concepts]]
