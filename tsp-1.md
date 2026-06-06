---
tags: cyber, plumb, soft3, token
alias: tsp1, tsp 1, coin, fungible token, fungible nature
crystal-type: spec
crystal-domain: cyber
---

# TSP-1 — Coin

the fungible token nature of [[plumb]]. one of two natures all value reduces to.

## nature

| property | value |
|---|---|
| fungibility | yes — units are interchangeable |
| conservation | Σ balances(class) = supply(class) |
| identity | per token class, not per unit |
| transfer | by [[pay]] (PLUMB) |
| creation | by [[mint]] (PLUMB) |
| destruction | by [[burn]] (PLUMB) |

a Coin has a class and a supply. holders carry a balance in that class. exactly one balance per (holder × Coin-class) pair.

## what a Coin represents

- currencies (CYB, USDT, IDR)
- continuous resources (kg of rice, kWh, GB-months, compute credits)
- shares of an entity (equity, voting weight)
- access credits (LLM calls, API quota, bandwidth)

## conservation

for every Coin class:

```
Σ balances(class) = mint_total(class) − burn_total(class)
```

mints and burns are explicit PLUMB operations between designated source and sink Cards. the equation holds by construction — violation is unprovable.

## relationship to Card

Coins are held BY Cards. a Card is a holder; a Coin balance is what it holds. accounts do not exist as a separate concept — what looks like an account is a Card with balances in some Coin classes.

## see also

- [[plumb/tsp-2]] — the non-fungible nature
- [[plumb/plumb]] — the five operations
- [[cyb/robot]] — the entity that holds Coins

---

discover all [[concepts]]
