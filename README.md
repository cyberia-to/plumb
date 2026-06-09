---
title: tok
tags: cyber, plumb, soft3, core
alias: Tok, token framework, plumb stack, value layer, plumb
crystal-type: spec
crystal-domain: cyber
icon: "🪠"
---

# tok

the value layer of [[soft3]]. three primitives that close the question of what value IS and how it MOVES in cyberspace:

| primitive | what it is |
|---|---|
| [[tsp-1\|Coin]] | the fungible token nature |
| [[tsp-2\|Card]] | the non-fungible token nature |
| PLUMB | the five operations: pay, lock, update, mint, burn |

every [[Sigma]] in cyberspace is a configuration of Coins held by Cards, mutated through PLUMB. every [[cyb/robot\|Robot]] is realized as a Card. every state change in cyberspace is a sequence of PLUMB operations.

## position in soft3

```
   neurons → Robots
                │
        tok  ───┤        value layer    — this repo
        bbg ────┤        memory layer
        tru ────┤        convergence layer
        radio ──┤        transport layer
        ...     │
            cybergraph
```

tok sits between the agency of the [[cyb/robot\|Robot]] (what it wants to do) and execution on the [[cybergraph]] (what was done). PLUMB programs compile through [[trident]], execute on [[nox]], and prove on [[zheng]].

## what makes tok minimal

- two natures, exactly: Coin (fungible) + Card (non-fungible). no third nature
- five operations, exactly: pay + lock + update + mint + burn. no sixth operation
- one composition rule: operations compose into [[Intent\|Intents]] that commit atomically or roll back

accounts, registries, ledgers, balance sheets, contracts, permits, escrows — all derived from Cards holding Coin balances mutated by PLUMB. if a behavior cannot be expressed in PLUMB, it does not belong in the value layer.

## conservation

PLUMB preserves four invariants by construction (the [[zheng]] proof system rejects any operation sequence that breaks them):

| law | statement |
|---|---|
| Sigma conservation | every pay has one source and one destination |
| Token conservation | Σ balances = mints − burns per Coin class |
| Card uniqueness | owner_count(id) = 1 always |
| Atomicity | all ops in an Intent commit together or none do |

provability replaces enforcement.

## see also

- [[cyb/robot]] — the entity that exercises PLUMB
- [[soft3]] — the stack tok belongs to
- [[bbg]] — where PLUMB-mutated state lives
- [[trident]] — the language tok operations compile from
- [[zheng]] — the proof system that enforces conservation
- [[nox]] — the VM that executes PLUMB

---

discover all [[concepts]]
