# plumb (tok) builds and tests clean against its siblings' origin default branches

date: 2026-09-23 · property #39 · repo: plumb (crate `cyber-tok`)

## claim

property 39 asks whether every phase-1 component builds and tests from a
clean checkout of its default branch against the default branches of its
siblings, with no dead path, no version pin behind a sibling, and no
crate present only in an owner's working tree. the 2026-09-22/23 sweep
listed bbg, cybergraph, foculus, mudra, neuron, nox, prysm, zheng,
true-cyber, glia, honeycrisp, rune, vault and radio as failing this gate.
plumb/tok was not on that list; this measures it directly rather than
assuming the omission means it passes.

## method

`origin/main` of plumb checked out in an isolated worktree
(`e27da0dff72b948418a5e40a5ed6290c0fd167f3`). `rs/Cargo.toml`'s three
path dependencies are [[neuron]]'s `id` crate, [[hemera]], and [[tru]].
neuron's and tru's owner working trees already matched their origin
default branches exactly (neuron at
`c87541120caae4fb5d5bb401653d1cbcbeffeb7d`, tru at
`89416291dd4cc993a79d63cbd3e2445294b173d3`, transitively pulling in
[[strata]] at `56aedb2d12b3126c601eb333419136d403614dbb`, itself
matching origin). hemera's owner tree carried unpushed local commits, so
it was substituted with a fresh detached worktree of `origin/main`
(`23f3bbcff910ea6d504ceb505680a539260869da`) for this run, so the result
reflects sibling default branches, not local owner-tree state.

worth recording since it looked like a live failure at first: `neuron-id`
being missing from a clean checkout is exactly the gap [neuron#1](https://github.com/cyberia-to/neuron/pull/1)
(row 39, still open) describes fixing — but `neuron-id` is already
present on neuron's `origin/main` (commit `3be383a`, "add neuron-id
crate"), landed before the sweep ran. plumb never hit the gap neuron#1
is naming.

## result

```
$ cargo check --tests --manifest-path rs/Cargo.toml
    Checking neuron-id v0.1.0
    Checking cyber-hemera v0.3.1
    Checking cyber-tru v0.1.1
    Checking cyber-tok v0.1.1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.71s

$ cargo test --manifest-path rs/Cargo.toml
running 8 tests
test conservation::tests::budget_caps_emission ... ok
test conservation::tests::clip_when_vstar_exceeds_delta ... ok
test conservation::tests::conservation_of_token_sum ... ok
test conservation::tests::no_clip_when_under_delta ... ok
test conservation::tests::zero_rho_style_share_gets_nothing ... ok
test ledger::tests::burn_preserves_conservation ... ok
test ledger::tests::mint_preserves_conservation ... ok
test mint::tests::execute_mints_under_clip ... ok
test result: ok. 8 passed; 0 failed; 0 ignored
```

8 tests pass, 0 fail, on `rustc 1.98.0 (88d9e12ae 2026-08-18)`. no dead
path dependency, no pin behind a sibling's published version, no crate
resolved only from an owner's working tree. plumb closes its row of
property 39.

## caveat

`origin/main` is behind plumb's own open launch PRs (#1–#16: transfer,
referral, ICBS market, stake-yield); this measures the base the sweep
would have measured, not the state after those merge. it also says
nothing about plumb's dependents resolving it correctly.
