# Cookie Agave Fork: Engineering Rules

Applies to every change in this repository, by any developer or AI assistant. Plan and design: Cookie Phase 1 documents (`cookie-chain-phase1-plan.md`, `cookie-chain-phase1-technical.md`).

The goal: Cookie changes should sit **beside** Agave's code, not inside it, so pulling in future Agave releases (4.3, 4.4 and on) produces few merge conflicts.

## 1. Put Cookie code in new files

- All Cookie logic lives in new files and modules, for example:
  - `runtime/src/cookie/` for runtime behaviour (fees, burn, stake cap, migrations)
  - `feature-set/src/cookie.rs` for Cookie feature IDs and the alias table
  - `programs/cookie-feature-gate/` for the feature switch program
- Upstream files get **only a one-line hook** that calls into the Cookie module, for example `if let Some(fee) = cookie::fees::override_fee(...) { ... }`.
- Never copy a large upstream function to modify it. Hook before or after it instead.

## 2. Never edit upstream identifiers

- Do not change any upstream `declare_id!`, feature ID, constant or public type.
- Upstream features are activated through the Cookie alias table, not by re-keying them.

## 2a. Keep every interface behaving like Solana

Cookie must feel seamless: users, operators and tools should not be able to tell that Cookie works differently inside.

- Commands (`solana`, `solana-test-validator`, `agave-ledger-tool`), RPC responses and anything else users touch must keep behaving exactly as on Solana, even when a Cookie change works differently underneath. Example: `solana feature status` shows a feature switched on through a Cookie alias as active, with its activation slot, just as Solana would.
- Before finishing a change, list every user-facing surface it affects and check each one. A "cosmetic" difference is a bug, not a footnote.
- Only break an interface when keeping it makes no sense, and say so explicitly in the pull request.
- Where several tools need the same Cookie logic, put it in one shared place (for example `feature-set/src/cookie.rs`, used by the runtime, the CLI, the test validator and ledger-tool) and give each tool only a one-line hook. This keeps behaviour consistent and merges small.

## 3. Every behaviour change is behind a Cookie feature

- With all Cookie features inactive, the build must produce identical bank hashes to stock Agave of the same version. This is what makes rolling upgrades safe.
- One feature per behaviour. No feature does two unrelated things.

## 4. Do not touch consensus internals

- No changes in `core/src/consensus*` (Tower BFT) or in Alpenglow's Votor code.
- Hook in at `EpochStakes`, fee calculation, fee distribution, rewards or epoch-boundary migrations instead.

## 5. Keep a touch-point list

- Maintain `COOKIE-TOUCHPOINTS.md` in the fork root: every upstream file Cookie edits, the line or function, and why.
- Every rebase starts by checking this list. The goal is to keep it short.
- Cookie merges newer Agave code constantly, so judge every change by the merge pain it will cause. Prefer adding a line over changing one, changing one line over several, and one shared helper over the same edit in many files.

## 6. Prefer upstream implementations

- Before writing a Cookie version of something, check whether a newer Agave release already has it. Fee sharing (SIMD-0123) is an example: its staker payout exists in 4.3.2.
- If Cookie needs something earlier than upstream ships it, match the upstream design and field names so Cookie can switch to upstream later.

## 7. Tests live in new files too

- Cookie tests go in `cookie/` test modules, not appended to upstream test files.
- Every Cookie feature has: a test that it does nothing while inactive, a test of its behaviour when active, and a test of the activation boundary.

## 8. One-time migrations are removable

- Code that runs once at activation (the wallet sweep, the stake program replacement) is isolated so it can be deleted in the next rebase, keeping only its feature ID.

## 9. Releases

- Branch per upstream version: `cookie/4.1.2`, later `cookie/4.3.x`.
- Cargo version follows Agave's patch bump. The client id is `Cookie` (`version/src/client_ids.rs`), so `solana --version` and gossip show `4.1.3` with `client:Cookie`. Git tags stay `<upstream>-cookie.<n>`, for example `4.1.2-cookie.1`.
- Portable builds (no `-C target-cpu=native`), published on GitHub with a SHA-256 checksum that is also posted separately in the operators' channel.
- Keep Anza's Apache 2.0 licence and notices.

## 10. Never

- Never plan or require a genesis reset. It breaks deployed programs.
- Never activate Alpenglow before Solana mainnet has run it safely.
- Never activate the validator admission ticket before every vote account has a BLS key.
