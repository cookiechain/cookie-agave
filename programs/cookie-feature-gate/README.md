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

- `Activate` fails if the alias is already a feature account. It still works if someone pre-funded the alias address, by topping it up to rent exemption.
- `Revoke` only works before the next epoch boundary. The feature program rejects revoking an active feature. The alias's lamports are burned.
- Config PDA: `["config"]`, 33 bytes: `[version = 1][authority]`.

## Deployment on Cookie (plan step 4)

1. Build: `cargo-build-sbf` (Agave 4.1.2 tools). Record the SHA-256 of `target/deploy/cookie_feature_gate.so`.
2. Deploy with the program keypair `CookmMGfe9…` (kept offline), with a temporary deployer as upgrade authority.
3. `Initialize` with `authority` = the Squads vault, signed by the temporary deployer.
4. `solana program set-upgrade-authority CookmMGfe9… --new-upgrade-authority <Squads vault>`.
5. Verify: config authority = Squads vault, upgrade authority = Squads vault, program hash matches the review.

From then on, every activation is a Squads vault transaction containing one `Activate` instruction. The vault can be both authority and payer.

## Tests

- Unit tests: `cargo test`
- End-to-end: `tests/e2e/run-e2e.sh cookie` and `tests/e2e/run-e2e.sh stock`. These start a local test validator, deploy the program, try every misuse, activate and revoke features, and check that the Cookie runtime (and only the Cookie runtime) activates the feature at the epoch boundary. Requires Agave 4.1.2 tools, Node.js 22 and `npm install` in `tests/e2e`. The `cookie` mode needs `cargo build -p agave-validator --bin solana-test-validator` at the repository root first.
