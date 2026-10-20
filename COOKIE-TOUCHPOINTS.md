# Cookie Touch Points

Every upstream Agave file that Cookie edits, and why. Keep this list short: Cookie logic belongs in new files (see `COOKIE-RULES.md`), with only one-line hooks in upstream files.

Check this list at the start of every rebase onto a newer Agave release.

| Upstream file | Function or line | Cookie change | Why | Added in |
|---|---|---|---|---|
| `runtime/src/lib.rs` | module list | `pub mod cookie;` | Registers the Cookie runtime module | C1 |
| `runtime/Cargo.toml` | `solana-pubkey` dependency | adds the `curve25519` feature | Alias PDA derivation (`find_program_address`) | C1 |
| `runtime/src/bank.rs` | `compute_and_apply_new_feature_activations` | reads the feature account via `cookie_feature_account` and stores the activation slot back to that address | Activation may live in a Cookie alias account | C1 |
| `runtime/src/bank.rs` | `compute_active_feature_set` | reads the feature account via `cookie_feature_account` | Same | C1 |
| `runtime/src/bank.rs` | `compute_pending_activation_slot` | reads the feature account via `cookie_feature_account` | Same | C1 |

New files (no merge conflicts expected): `runtime/src/cookie/`, `programs/cookie-feature-gate/`, `COOKIE-RULES.md`, `COOKIE-TOUCHPOINTS.md`.

Not changed on purpose: command-line and test tools that read feature accounts directly (`cli`, `ledger-tool`, `test-validator`, `genesis`, `local-cluster`). They do not see aliases, so `solana feature status` shows an alias-activated feature as inactive. This is cosmetic and does not affect consensus.

## Release history

| Release | Upstream base | Contents |
|---|---|---|
| `4.1.2-cookie.1` | `v4.1.2` (`182084b82aae88b1b0731540f63bf7416f27773a`) | C1: Cookie feature switches (alias table, runtime hooks, `cookie-feature-gate` program) |
