# cookie-feature-gate

Cookie Chain's feature switch program. Program ID `CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4`.

Stock Agave can only activate a feature if someone signs with that feature's private key, and Anza holds those keys. This program lets the Cookie Squads multisig activate features instead.

## How it works

1. For an upstream feature ID `F`, the program creates a pending feature account (owned by `Feature111…`, 9 zero bytes) at the alias PDA `["cookie-alias", F]`.
2. The Cookie validator release reads that alias (`runtime/src/cookie/feature_alias.rs`) and activates `F` at the next epoch boundary, exactly as if `F`'s own account had been created.
3. Stock Agave ignores the alias, so nothing happens on a validator that has not upgraded.

Only this program can sign for its PDAs, so only this program can create an alias. Only the configured authority (the Squads vault) can tell it to.

## Instructions

Each instruction is one tag byte followed by one 32-byte public key. Builders are in `src/instruction.rs`.

| Tag | Instruction | Signer | Accounts |
|---|---|---|---|
| 0 | `Initialize { authority }` | This program's upgrade authority (one time only) | config (w), payer (w,s), upgrade authority (s), program data, system program |
| 1 | `SetAuthority { new_authority }` | Current authority | config (w), authority (s) |
| 2 | `Activate { feature_id }` | Authority | config, authority (s), alias (w), payer (w,s), system program |
| 3 | `Revoke { feature_id }` | Authority | config, authority (s), alias (w), incinerator (w), feature program, system program |
| 4 | `AcceptAuthority { new_authority }` | The pending authority | config (w), new authority (s) |

- `Activate` fails if the alias is already a feature account. It still works if someone pre-funded the alias address, by topping it up to rent exemption.
- `Activate` refuses the four denylisted IDs (`src/denylist.rs`): the Alpenglow placeholder in Agave 4.1 and three IDs upstream abandoned. The validator ignores their aliases anyway; refusing them here means a mistaken request cannot sit pending and switch on by itself in a later release. The list must match `DENYLIST` in `feature-set/src/cookie.rs`, and a test there checks it.
- Handing over the authority takes two steps, so a mistake cannot hand it to an address nobody controls. `SetAuthority` names a pending authority; the current authority stays in charge until the pending one signs `AcceptAuthority`. `SetAuthority` naming the current authority clears a pending handover. This works between any two signers, including vaults of different multisigs, which could not both sign one transaction.
- `Revoke` only works before the next epoch boundary. The feature program rejects revoking an active feature. The alias's lamports are burned.
- Config PDA: `["config"]`, 66 bytes: `[version = 2][authority][has_pending: 0 or 1][pending authority]`. Version 1 (33 bytes, single-step handover) was never deployed.

## Deployment on Cookie (plan step 4)

Authority (decided 20 October 2026): **vault 3 of the community multisig**.

| | Address |
|---|---|
| Community multisig (Squads v4, 6 of 12, signed at sig.cookiechain.wtf/community) | `BQVAdo6Ls2rYAggzXu38peyYg8BtQASUn6bnm9FfS5Ab` |
| Vault 3: switch authority and upgrade authority | `AEMq4AZiGU2smZJX4Y3hqrSmAocGJzBAyvNDWSQeQ4Xe` |

1. Fund vault 3 with about 1 COOK. Each activation costs about 0.00095 COOK of rent plus fees, and the vault pays.
2. Build: `cargo-build-sbf` (Agave 4.1.2 tools). Check the SHA-256 of `target/deploy/cookie_feature_gate.so` matches the reviewed build.
3. Deploy with the program keypair `CookmMGfe9…` (kept offline), with a temporary deployer as upgrade authority.
4. `Initialize` with `authority` = `AEMq4AZiGU2smZJX4Y3hqrSmAocGJzBAyvNDWSQeQ4Xe`, signed by the temporary deployer.
5. `solana program set-upgrade-authority CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4 --new-upgrade-authority AEMq4AZiGU2smZJX4Y3hqrSmAocGJzBAyvNDWSQeQ4Xe --skip-new-upgrade-authority-signer-check`.
6. Verify: config authority = vault 3, upgrade authority = vault 3, program hash matches the review.

From then on, every activation is a vault 3 transaction of the community multisig containing one or more `Activate` instructions. Vault 3 is both authority and payer. Signers approve and execute it at sig.cookiechain.wtf/community like any other proposal.

## Tests

- Unit tests: `cargo test`
- End-to-end: `tests/e2e/run-e2e.sh cookie` and `tests/e2e/run-e2e.sh stock`. These start a local test validator, deploy the program, try every misuse, activate and revoke features, and check that the Cookie runtime (and only the Cookie runtime) activates the feature at the epoch boundary. In `cookie` mode they also check that `solana feature status` reports the feature as active exactly as on Solana, and that `solana-test-validator --clone-feature-set` copies it. Requires Agave 4.1.2 tools, Node.js 22 and `npm install` in `tests/e2e`. The `cookie` mode needs `cargo build -p agave-validator --bin solana-test-validator -p solana-cli --bin solana` at the repository root first.
- Ledger tool: `tests/e2e/ledger-tool-check.sh` checks that `agave-ledger-tool create-snapshot --deactivate-feature-gate` (used in an emergency restart) works for a feature activated through an alias. Build `agave-ledger-tool` in `dev-bins` first.

## Viewing feature status

Use the Cookie build of the `solana` CLI. `solana feature status` shows features activated through the switches exactly as Solana shows normal activations. The stock Solana CLI cannot see Cookie aliases and shows them as inactive.
