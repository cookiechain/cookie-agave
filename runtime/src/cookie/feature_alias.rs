//! Cookie feature aliases.
//!
//! Solana features are activated by creating an account at the feature's own
//! address, which needs that address's private key. Anza holds those keys, so
//! stock Agave cannot activate anything on Cookie Chain.
//!
//! Cookie gives every upstream feature a second address: a program-derived
//! address (PDA) of the `cookie-feature-gate` program, which is controlled by
//! the Cookie Squads multisig. When the multisig activates a feature, the
//! program creates a standard pending feature account at that PDA. The bank
//! then treats the upstream feature as activated exactly as if its own
//! account had been created.
//!
//! Rules:
//! - Upstream feature IDs are never edited. Upstream code keeps checking
//!   `feature_set.is_active(&upstream::id())`.
//! - If an upstream feature account exists, it is used as on stock Agave and
//!   the alias is ignored.
//! - An alias only counts if it is a valid feature account, which means it is
//!   owned by the feature program. Only `cookie-feature-gate` can sign for
//!   its PDAs, so only it can create one.
//! - Features in [`DENYLIST`] have no alias and can never be activated this
//!   way.
//!
//! With no alias accounts on chain this module changes nothing, so a release
//! containing it behaves exactly like stock Agave until the first activation.

use {
    crate::bank::Bank,
    agave_feature_set::{self as feature_set, FEATURE_NAMES},
    ahash::AHashMap,
    solana_account::AccountSharedData,
    solana_feature_gate_interface as feature,
    solana_pubkey::{Pubkey, pubkey},
    std::sync::LazyLock,
};

/// Program ID of the Cookie feature switch program.
pub const COOKIE_FEATURE_GATE_PROGRAM_ID: Pubkey =
    pubkey!("CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4");

/// Seed prefix for alias PDAs: `[ALIAS_SEED, upstream_feature_id]`.
pub const ALIAS_SEED: &[u8] = b"cookie-alias";

/// Upstream features that must never be activated through a Cookie alias.
///
/// - Alpenglow (SIMD-0326) is only a placeholder in Agave 4.1. It is
///   activated on Cookie's 4.3 release, after Solana mainnet runs it safely.
/// - The other three were abandoned upstream.
pub static DENYLIST: LazyLock<[Pubkey; 4]> = LazyLock::new(|| {
    [
        feature_set::alpenglow::id(),
        feature_set::enable_loader_v4::id(),
        feature_set::enable_extend_program_checked::id(),
        feature_set::stake_minimum_delegation_for_rewards::id(),
    ]
});

/// Alias PDA for an upstream feature ID.
pub fn alias_address(feature_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[ALIAS_SEED, feature_id.as_ref()],
        &COOKIE_FEATURE_GATE_PROGRAM_ID,
    )
    .0
}

/// Upstream feature ID to alias PDA, for every known feature except the
/// denylist. Computed once.
static ALIASES: LazyLock<AHashMap<Pubkey, Pubkey>> = LazyLock::new(|| {
    FEATURE_NAMES
        .keys()
        .filter(|id| !DENYLIST.contains(id))
        .map(|id| (*id, alias_address(id)))
        .collect()
});

/// The alias PDA for `feature_id`, if it has one.
pub fn alias_of(feature_id: &Pubkey) -> Option<&'static Pubkey> {
    ALIASES.get(feature_id)
}

impl Bank {
    /// The account that holds activation state for `feature_id`, and its
    /// address.
    ///
    /// The upstream feature account wins if it exists (stock behaviour).
    /// Otherwise the Cookie alias is used, but only if it is a valid feature
    /// account.
    pub(crate) fn cookie_feature_account(
        &self,
        feature_id: &Pubkey,
    ) -> Option<(Pubkey, AccountSharedData)> {
        if let Some(account) = self.get_account_with_fixed_root(feature_id) {
            return Some((*feature_id, account));
        }
        let alias = alias_of(feature_id)?;
        let account = self.get_account_with_fixed_root(alias)?;
        feature::from_account(&account)?;
        Some((*alias, account))
    }
}
