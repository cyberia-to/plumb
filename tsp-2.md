---
tags: cyber, plumb, soft3, token
alias: tsp2, tsp 2, card, non-fungible token, non-fungible nature
crystal-type: spec
crystal-domain: cyber
---

# TSP-2 — Card

the non-fungible token nature of [[plumb]]. one of two natures all value reduces to.

## nature

| property | value |
|---|---|
| fungibility | no — each unit has identity |
| conservation | owner_count(id) = 1 always |
| identity | per Card, content-addressed |
| transfer | by [[update]] (PLUMB) ownership change |
| creation | by [[mint]] (PLUMB) |
| destruction | by [[burn]] (PLUMB) |

a Card has an id, an owner, a configurable trait profile, and holds Coin balances. exactly one Card exists per id; exactly one owner per Card at every block.

## what a Card represents

- persons (Robots — with [[Soul]] and [[Avatar]])
- places (slots, parcels, buildings, time-windows)
- contracts (commitments, agreements, leases)
- titles (deeds, registrations, certificates)
- credentials (passports, permits, licenses, diplomas)
- assets (anything sold via [[cyberia/protocol/marketplace]])

## conservation

```
∀ id : owner_count(id) = 1
```

ownership is single-valued at every block. transfer is an atomic ownership-change PLUMB operation through `update`.

## relationship to Coin

Cards HOLD Coin balances. a Card's economic state is its set of (Coin-class, balance) pairs plus the bonds it carries to other Cards. inventory, registry, balance sheet — all views over Card state.

## the Robot is a Card

every [[cyb/robot|Robot]] is realized as a Card. its [[Soul]] points to the Card; its [[Sigma]] is the set of Coin balances the Card holds and the bonds it carries to other Cards.

## traits — the configurable profile

a Card carries a trait profile in five categories (see [[cyb/robot]] §6 for the accounting projection):

- skills — what the Card CAN do
- duties — what the Card MUST do (or cannot do)
- senses — what the Card PERCEIVES
- bonds — what the Card is connected to
- memory — what the Card has accumulated

traits install through PLUMB hooks via the `update` operation.

## see also

- [[plumb/tsp-1]] — the fungible nature
- [[plumb/plumb]] — the five operations
- [[cyb/robot]] — the entity that IS a Card

---

discover all [[concepts]]
