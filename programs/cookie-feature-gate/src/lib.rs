//! # cookie-feature-gate
//!
//! Cookie Chain's feature switch program. The Cookie Squads multisig uses it
//! to activate (and, before the epoch boundary, cancel) features on Cookie
//! Chain without holding any feature's private key.
//!
//! For an upstream feature ID `F`, the program creates a standard pending
//! feature account at the alias PDA `["cookie-alias", F]`. The Cookie
//! validator release (`runtime/src/cookie/feature_alias.rs`) treats that
//! account as the activation request for `F`.
//!
//! Instructions (first byte):
//! - `0` Initialize { authority }: one time, signed by this program's
//!   upgrade authority. Sets who may activate features.
//! - `1` SetAuthority { new_authority }: signed by the current authority.
//! - `2` Activate { feature_id }: signed by the authority.
//! - `3` Revoke { feature_id }: signed by the authority. Only works while the
//!   activation is still pending (before the next epoch boundary).

#![allow(unexpected_cfgs)]

pub mod instruction;
pub mod processor;
pub mod state;

solana_program::declare_id!("CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4");

/// Seed for the single config account.
pub const CONFIG_SEED: &[u8] = b"config";
/// Seed prefix for feature alias accounts: `[ALIAS_SEED, feature_id]`.
pub const ALIAS_SEED: &[u8] = b"cookie-alias";

/// The native feature program that owns feature accounts.
pub const FEATURE_PROGRAM_ID: solana_program::pubkey::Pubkey =
    solana_program::pubkey!("Feature111111111111111111111111111111111111");
/// Feature accounts are 9 bytes: `Option<u64>` activation slot.
pub const FEATURE_ACCOUNT_SIZE: u64 = 9;
/// The upgradeable BPF loader, which records this program's upgrade authority.
pub const BPF_LOADER_UPGRADEABLE_ID: solana_program::pubkey::Pubkey =
    solana_program::pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");
/// Lamports of revoked feature accounts are burned here.
pub const INCINERATOR_ID: solana_program::pubkey::Pubkey =
    solana_program::pubkey!("1nc1nerator11111111111111111111111111111111");

/// Config PDA address and bump.
pub fn config_address() -> (solana_program::pubkey::Pubkey, u8) {
    solana_program::pubkey::Pubkey::find_program_address(&[CONFIG_SEED], &id())
}

/// Alias PDA address and bump for an upstream feature ID.
pub fn alias_address(
    feature_id: &solana_program::pubkey::Pubkey,
) -> (solana_program::pubkey::Pubkey, u8) {
    solana_program::pubkey::Pubkey::find_program_address(&[ALIAS_SEED, feature_id.as_ref()], &id())
}

#[cfg(not(feature = "no-entrypoint"))]
use processor::process_instruction;
#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);
