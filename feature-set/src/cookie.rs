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
//! - The upstream address and the alias are ranked by [`FeatureRecord`]: an
//!   activated feature account beats a pending one, which beats anything
//!   else. On a tie the upstream address wins, as on stock Agave. So a plain
//!   account at the upstream address (anyone can send lamports to any
//!   address) does not hide the alias, and a pending account someone later
//!   creates there with the feature's own key does not hide an activated
//!   alias, which a restarting validator would otherwise read as inactive.
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
///
/// The `cookie-feature-gate` program refuses to activate these too, from its
/// own copy in `programs/cookie-feature-gate/src/denylist.rs`. A test below
/// fails if the two lists differ.
pub static DENYLIST: LazyLock<[Pubkey; 4]> = LazyLock::new(|| {
    [
        crate::alpenglow::id(),
        crate::enable_loader_v4::id(),
        crate::enable_extend_program_checked::id(),
        crate::stake_minimum_delegation_for_rewards::id(),
    ]
});

/// Cookie Chain only: replace SPL Token with p-token from the Cookie buffer.
/// Activated through the Cookie feature switch, which derives this address, so
/// no keypair for it exists.
pub mod replace_spl_token_with_p_token {
    solana_pubkey::declare_id!("CDJ9yJcfcJ2QkHaYhseA7jtihbhmNsBK46tZAJGsN9Xg");
}

/// Cookie-only features, with their descriptions. Chained into
/// [`crate::FEATURE_NAMES`] by one line, so they count as known features and
/// get an alias like any upstream feature, without editing upstream's list.
pub fn feature_names() -> impl Iterator<Item = (Pubkey, &'static str)> {
    [(
        replace_spl_token_with_p_token::id(),
        "Cookie: Replace SPL Token with p-token from the Cookie buffer",
    )]
    .into_iter()
}

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

/// How an account records a feature. Used to choose between a feature's
/// upstream address and its alias; later variants win.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatureRecord {
    /// No account, or an account that is not a feature account.
    NotAFeature,
    /// A feature account with no activation slot yet.
    Pending,
    /// A feature account with an activation slot.
    Activated,
}

/// Whether the alias decides the feature's state rather than the upstream
/// address: only if it records the feature more strongly. On a tie the
/// upstream address wins, as on stock Agave.
pub fn alias_wins(upstream: FeatureRecord, alias: FeatureRecord) -> bool {
    alias > upstream
}

/// For client tools that fetch feature accounts in bulk: the positions in
/// `fetched` whose upstream account is not an activated feature account and
/// which have an alias, and the alias addresses to fetch for them, in the
/// same order.
pub fn missing_aliases<T>(
    ids: &[Pubkey],
    fetched: &[Option<T>],
    record: impl Fn(&T) -> FeatureRecord,
) -> (Vec<usize>, Vec<Pubkey>) {
    ids.iter()
        .zip(fetched)
        .enumerate()
        .filter(|(_, (_, account))| {
            account.as_ref().map_or(FeatureRecord::NotAFeature, &record) != FeatureRecord::Activated
        })
        .filter_map(|(position, (id, _))| alias_of(id).map(|alias| (position, *alias)))
        .unzip()
}

/// Fills the positions returned by [`missing_aliases`] with the fetched alias
/// accounts where [`alias_wins`]. Otherwise the upstream entry is kept.
pub fn fill_from_aliases<T>(
    fetched: &mut [Option<T>],
    positions: &[usize],
    alias_accounts: Vec<Option<T>>,
    record: impl Fn(&T) -> FeatureRecord,
) {
    for (position, alias_account) in positions.iter().zip(alias_accounts) {
        let entry = &mut fetched[*position];
        let upstream = entry.as_ref().map_or(FeatureRecord::NotAFeature, &record);
        let alias = alias_account
            .as_ref()
            .map_or(FeatureRecord::NotAFeature, &record);
        if alias_wins(upstream, alias) {
            *entry = alias_account;
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

    /// In these tests an account is a number: 0 is not a feature account,
    /// 1 is a pending feature account, 2 is an activated one.
    const PLAIN: u8 = 0;
    const PENDING: u8 = 1;
    const ACTIVATED: u8 = 2;

    fn record(account: &u8) -> FeatureRecord {
        match *account {
            ACTIVATED => FeatureRecord::Activated,
            PENDING => FeatureRecord::Pending,
            _ => FeatureRecord::NotAFeature,
        }
    }

    #[test]
    fn alias_wins_only_when_it_records_the_feature_more_strongly() {
        use FeatureRecord::*;
        assert!(alias_wins(NotAFeature, Pending));
        assert!(alias_wins(NotAFeature, Activated));
        assert!(alias_wins(Pending, Activated));
        // Ties and weaker aliases leave the upstream address in charge.
        assert!(!alias_wins(Pending, Pending));
        assert!(!alias_wins(Activated, Activated));
        assert!(!alias_wins(Activated, Pending));
        assert!(!alias_wins(NotAFeature, NotAFeature));
    }

    #[test]
    fn missing_aliases_for_aliasable_features_not_yet_activated_upstream() {
        let with_alias = crate::block_revenue_sharing::id();
        let denied = crate::alpenglow::id();
        let unknown = Pubkey::new_unique();
        let ids = [
            with_alias, with_alias, with_alias, with_alias, denied, unknown,
        ];
        let fetched = [
            None,
            Some(ACTIVATED),
            Some(PLAIN),
            Some(PENDING),
            None,
            Some(PLAIN),
        ];
        let (positions, aliases) = missing_aliases(&ids, &fetched, record);
        assert_eq!(positions, vec![0, 2, 3]);
        assert_eq!(aliases, vec![alias_address(&with_alias); 3]);
    }

    #[test]
    fn fill_uses_the_alias_only_where_it_wins() {
        let mut fetched = vec![
            None,
            Some(PLAIN),
            Some(PENDING),
            Some(PENDING),
            Some(PLAIN),
            Some(ACTIVATED),
        ];
        let aliases = vec![
            Some(PENDING),
            Some(ACTIVATED),
            Some(ACTIVATED),
            Some(PENDING),
            Some(PLAIN),
            Some(ACTIVATED),
        ];
        fill_from_aliases(&mut fetched, &[0, 1, 2, 3, 4, 5], aliases, record);
        assert_eq!(
            fetched,
            vec![
                Some(PENDING),   // nothing upstream: the pending alias counts
                Some(ACTIVATED), // a plain account upstream does not hide it
                Some(ACTIVATED), // an activated alias beats a pending upstream account
                Some(PENDING),   // a tie keeps the upstream account
                Some(PLAIN),     // a plain alias never replaces anything
                Some(ACTIVATED), // an activated upstream account is never replaced
            ]
        );
    }

    // With `dev-context-only-utils`, `alpenglow::id()` is a test key, not the
    // real placeholder the program refuses, so this only runs in normal builds.
    #[cfg(not(feature = "dev-context-only-utils"))]
    #[test]
    fn denylist_matches_the_switch_program() {
        let program_list = include_str!("../../programs/cookie-feature-gate/src/denylist.rs");
        let mut in_program: Vec<String> = program_list
            .split("pubkey!(\"")
            .skip(1)
            .map(|rest| rest.split('"').next().unwrap().to_string())
            .collect();
        let mut here: Vec<String> = DENYLIST.iter().map(|id| id.to_string()).collect();
        in_program.sort();
        here.sort();
        assert_eq!(
            in_program, here,
            "programs/cookie-feature-gate/src/denylist.rs and DENYLIST differ"
        );
    }
}
