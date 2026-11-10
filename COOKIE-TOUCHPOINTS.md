# Cookie Touch Points

Every upstream Agave file that Cookie edits, and why. Keep this list short: Cookie logic belongs in new files (see `COOKIE-RULES.md`), with only one-line hooks in upstream files.

Check this list at the start of every rebase onto a newer Agave release.

| Upstream file | Function or line | Cookie change | Why | Added in |
|---|---|---|---|---|
| `feature-set/src/lib.rs` | end of file | `pub mod cookie;` | Shared Cookie feature alias table | C1 |
| `feature-set/Cargo.toml` | `solana-pubkey` dependency | adds the `curve25519` feature | Alias PDA derivation (`find_program_address`) | C1 |
| `runtime/src/lib.rs` | module list | `pub mod cookie;` | Cookie runtime module | C1 |
| `runtime/src/bank.rs` | `compute_and_apply_new_feature_activations` | reads the feature account via `cookie_feature_account` and stores the activation slot back to that address | Activation may live in a Cookie alias account | C1 |
| `runtime/src/bank.rs` | `compute_active_feature_set` | reads the feature account via `cookie_feature_account` | Same | C1 |
| `runtime/src/bank.rs` | `compute_pending_activation_slot` | reads the feature account via `cookie_feature_account` | Same | C1 |
| `cli/src/lib.rs` | module list | `pub mod cookie;` | Cookie CLI helpers | C1 |
| `cli/src/feature.rs` | imports; `get_feature_status`, `get_feature_activation_epoch`, `process_status`, `process_activate`, `process_revoke` | one import line (`cookie::CookieFeatureAccounts`); method name swaps `.get_account` to `.get_feature_account` and `.get_multiple_accounts` to `.get_feature_accounts`; one added line in `process_revoke` (`cookie::check_revoke_target`) | `solana feature status` / `activate` and every command that checks feature status behave as on Solana. `revoke` explains that a switch request is withdrawn through the multisig, since only the switch program can sign for an alias | C1 |
| `cli/src/program.rs` | imports; `fetch_feature_set` | one import line (`cookie::CookieFeatureAccounts`); method name swap `.get_multiple_accounts` to `.get_feature_accounts` | `program deploy`, `upgrade`, `write-buffer` and `extend` check programs against the features actually active on Cookie | C1 |
| `cli/src/vote.rs` | imports; `process_show_vote_account` (timely vote credits lookup) | one import line; method name swap `.get_account_with_commitment` to `.get_feature_account_with_commitment` | Same | C1 |
| `test-validator/src/lib.rs` | top of file; `clone_feature_set` | `mod cookie;` and one added call after the loop | `--clone-feature-set` copies Cookie alias activations | C1 |
| `ledger-tool/src/main.rs` | `create-snapshot` feature-gate deactivation | look up and clear the feature via `cookie_feature_account` | Emergency feature deactivation also works for alias-activated features | C1 |

New files (no merge conflicts expected): `feature-set/src/cookie.rs`, `runtime/src/cookie/`, `cli/src/cookie.rs`, `test-validator/src/cookie.rs`, `programs/cookie-feature-gate/`, `COOKIE-RULES.md`, `COOKIE-TOUCHPOINTS.md`.

## Known differences from Solana

| Surface | Difference | Why left |
|---|---|---|
| `RpcClient::get_feature_activation_slot` (Rust `solana-rpc-client` library) | Does not see Cookie aliases | Not used anywhere in Agave; hooking it would add an `agave-feature-set` dependency to the published RPC client crate. Rust apps should use the CLI helpers' approach. |
| `solana-genesis`, `local-cluster` | Do not see Cookie aliases | Only used to create new clusters and for tests. Cookie never resets genesis. |

## Release history

| Release | Upstream base | Contents |
|---|---|---|
| `4.1.2-cookie.1` | `v4.1.2` (`182084b82aae88b1b0731540f63bf7416f27773a`) | C1: Cookie feature switches (alias table, runtime and tool hooks, `cookie-feature-gate` program) |
