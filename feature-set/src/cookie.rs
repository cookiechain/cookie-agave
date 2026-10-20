//! Cookie Chain feature aliases.
//!
//! Solana features are activated by creating an account at the feature's own
//! address, which needs that address's private key. Anza holds those keys, so
//! stock Agave cannot activate anything on Cookie Chain.
//!
//! Cookie gives every upstream feature a second address: a program-derived
//! address (PDA) of the `cookie-feature-gate` program, which is controlled by
//! the Cookie Squads multisig. When the multisig activates a feature, the
//! program creates a standard pending feature account at that PDA. The
//! runtime, the CLI and the other tools then treat the upstream feature
//! exactly as if its own account had been created, so everything behaves as
//! on Solana.
//!
//! This module is the single source of truth for alias addresses. It is
//! shared by the runtime (`runtime/src/cookie/`) and the client tools
//! (`cli/src/cookie.rs`, `test-validator/src/cookie.rs`, `ledger-tool`).
//!
//! Rules:
//! - Upstream feature IDs are never edited.
//! - If an upstream feature account exists, it is used as on stock Agave and
//!   the alias is ignored.
//! - An alias only counts if it is a valid feature account.
//! - Features in [`DENYLIST`] have no alias.

use {
    crate::FEATURE_NAMES,
    ahash::AHashMap,
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
        crate::alpenglow::id(),
        crate::enable_loader_v4::id(),
        crate::enable_extend_program_checked::id(),
        crate::stake_minimum_delegation_for_rewards::id(),
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

/// For client tools that fetch feature accounts in bulk: the positions in
/// `fetched` whose account is missing but which have an alias, and the alias
/// addresses to fetch for them, in the same order.
pub fn missing_aliases<T>(ids: &[Pubkey], fetched: &[Option<T>]) -> (Vec<usize>, Vec<Pubkey>) {
    ids.iter()
        .zip(fetched)
        .enumerate()
        .filter(|(_, (_, account))| account.is_none())
        .filter_map(|(position, (id, _))| alias_of(id).map(|alias| (position, *alias)))
        .unzip()
}

/// Fills the positions returned by [`missing_aliases`] with the fetched alias
/// accounts, keeping only those that `is_feature_account` accepts.
pub fn fill_from_aliases<T>(
    fetched: &mut [Option<T>],
    positions: &[usize],
    alias_accounts: Vec<Option<T>>,
    is_feature_account: impl Fn(&T) -> bool,
) {
    for (position, alias_account) in positions.iter().zip(alias_accounts) {
        if fetched[*position].is_none() {
            fetched[*position] = alias_account.filter(|account| is_feature_account(account));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_cover_every_feature_except_the_denylist() {
        for id in FEATURE_NAMES.keys() {
            match alias_of(id) {
                Some(alias) => {
                    assert!(!DENYLIST.contains(id));
                    assert_eq!(*alias, alias_address(id));
                    assert!(!alias.is_on_curve(), "alias {alias} must be a PDA");
                }
                None => assert!(DENYLIST.contains(id), "{id} has no alias"),
            }
        }
        for id in DENYLIST.iter() {
            assert!(FEATURE_NAMES.contains_key(id));
        }
    }

    #[test]
    fn missing_aliases_only_for_missing_aliasable_features() {
        let with_alias = crate::block_revenue_sharing::id();
        let denied = crate::alpenglow::id();
        let unknown = Pubkey::new_unique();
        let ids = [with_alias, with_alias, denied, unknown];
        let fetched = [None, Some(1u8), None, None];
        let (positions, aliases) = missing_aliases(&ids, &fetched);
        assert_eq!(positions, vec![0]);
        assert_eq!(aliases, vec![alias_address(&with_alias)]);
    }

    #[test]
    fn fill_keeps_only_valid_feature_accounts() {
        let mut fetched = vec![None, Some(7u8), None];
        fill_from_aliases(&mut fetched, &[0, 2], vec![Some(1u8), Some(2u8)], |a| {
            *a == 1
        });
        assert_eq!(fetched, vec![Some(1), Some(7), None]);
    }
}
