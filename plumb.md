---
tags: cyber, plumb, soft3, operations
alias: plumb ops, plumb operations, five operations
crystal-type: spec
crystal-domain: cyber
---

# PLUMB — the five operations

every state change in cyberspace reduces to one of five atomic operations on [[plumb/tsp-1|Coins]] and [[plumb/tsp-2|Cards]]:

| op | what it does | acts on |
|---|---|---|
| pay | transfer Coin balance between Cards | Coin |
| lock | constrain a Token (install a Sensor, set a floor, freeze) | Coin or Card |
| update | change configuration (rotate authority, install or remove traits, change owner) | Card |
| mint | create a new Token instance | Coin class or Card |
| burn | destroy a Token instance | Coin class or Card |

every operation has hooks where [[Sensor|Sensors]] install. a Sensor with reaction=Block applied to a hook gates the operation: `lock` is just `update` with a Block Sensor installed.

## atomic composition: Intent

an [[Intent]] is one or more PLUMB operations composed atomically. all operations commit together, or none commit.

```
intent: book_banya {
  pay  user → asset_owner      0.05 CYB
  mint claim                   to user
}
```

both ops succeed together, or neither does. the workflow that owns the Intent reserves inputs at submission, locks balances during pending, releases at success or rollback at failure.

## what does NOT need to be a PLUMB operation

every higher-level transaction is a composition of these five. there is no sixth:

- transfer = pay
- approval = update (install allowance trait)
- escrow = pay + lock + update (or [[CommitmentGuard]] which avoids escrow)
- airdrop = mint per recipient
- vesting = mint + update (install schedule trait)
- governance vote = mint + burn (mint ballot, burn after counting)
- subscription = pay on Schedule

if a behavior cannot be expressed as a PLUMB composition, it does not belong in the value layer.

## hooks

each operation exposes hooks where Sensors install:

- pay → pay_hook (fires before transfer; Block reaction rejects)
- lock → lock_hook (fires before constraint application)
- update → update_hook (fires before configuration change)
- mint → mint_hook (fires before creation; Block reaction prevents minting)
- burn → burn_hook (fires before destruction; Block reaction prevents burning)

hooks are how compliance, rate limits, royalties, allowances, commitment guards, and audience gates compose with the operations.

## conservation

PLUMB preserves four laws by construction (see [[cyb/robot]] §conservation):

1. Sigma conservation — pay has one source and one destination
2. Token conservation — Σ balances = mints − burns per Coin class
3. Card uniqueness — owner_count(id) = 1 at every block
4. Atomicity — all ops in an Intent commit together or none do

violations are unprovable: the [[zheng]] proof system rejects any operation sequence that breaks them.

## see also

- [[plumb/tsp-1]] — Coin nature
- [[plumb/tsp-2]] — Card nature
- [[cyb/robot]] — the entity that issues PLUMB operations
- [[trident]] — the language PLUMB compiles from
- [[zheng]] — the proof system enforcing conservation
- [[nox]] — the VM that executes PLUMB

---

discover all [[concepts]]
