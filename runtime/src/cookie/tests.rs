//! Tests for Cookie runtime extensions.

use {
    super::feature_alias::*,
    crate::{
        bank::Bank,
        bank_forks::BankForks,
        genesis_utils::create_genesis_config,
        runtime_config::RuntimeConfig,
        snapshot_bank_utils::{bank_from_snapshot_archives, bank_to_full_snapshot_archive},
        snapshot_utils::create_tmp_accounts_dir_for_tests,
    },
    agave_feature_set::{self as feature_set, FEATURE_NAMES},
    agave_snapshots::snapshot_config::SnapshotConfig,
    solana_account::{AccountSharedData, ReadableAccount},
    solana_accounts_db::accounts_db::ACCOUNTS_DB_CONFIG_FOR_TESTING,
    solana_feature_gate_interface::{self as feature, Feature},
    solana_hash::Hash,
    solana_leader_schedule::SlotLeader,
    solana_pubkey::Pubkey,
    std::sync::{Arc, RwLock},
    tempfile::TempDir,
};

/// A real upstream feature that is not denylisted, used as the test subject.
fn subject() -> Pubkey {
    feature_set::block_revenue_sharing::id()
}

/// A test bank in which `feature_id` is not active.
fn bank_without(feature_id: &Pubkey) -> (Arc<Bank>, Arc<RwLock<BankForks>>) {
    let mut info = create_genesis_config(1_000_000_000_000);
    info.genesis_config.accounts.remove(feature_id);
    let bank = Bank::new_for_tests(&info.genesis_config);
    assert!(!bank.feature_set.is_active(feature_id));
    bank.wrap_with_bank_forks_for_tests()
}

/// The first bank of the next epoch, where pending features activate.
fn next_epoch(parent: Arc<Bank>) -> (Bank, u64) {
    let slot = parent
        .epoch_schedule()
        .get_first_slot_in_epoch(parent.epoch() + 1);
    (
        Bank::new_from_parent(parent, SlotLeader::default(), slot),
        slot,
    )
}

/// A pending feature account, as `cookie-feature-gate` creates it.
fn pending_feature_account() -> AccountSharedData {
    feature::create_account(&Feature::default(), 1_000_000_000)
}

fn activated_at(bank: &Bank, address: &Pubkey) -> Option<u64> {
    feature::from_account(&bank.get_account(address)?)?.activated_at
}

#[test]
fn aliases_cover_every_feature_except_the_denylist() {
    for id in FEATURE_NAMES.keys() {
        match alias_of(id) {
            Some(alias) => {
                assert!(!DENYLIST.contains(id), "denylisted {id} has an alias");
                assert_eq!(*alias, alias_address(id));
                assert_ne!(alias, id);
                assert!(!alias.is_on_curve(), "alias {alias} must be a PDA");
            }
            None => assert!(DENYLIST.contains(id), "{id} has no alias"),
        }
    }
    for id in DENYLIST.iter() {
        assert!(
            FEATURE_NAMES.contains_key(id),
            "denylist entry {id} is not a known feature"
        );
        assert!(alias_of(id).is_none());
    }
}

#[test]
fn alias_address_is_the_documented_pda() {
    let id = subject();
    let (expected, _bump) = Pubkey::find_program_address(
        &[b"cookie-alias", id.as_ref()],
        &COOKIE_FEATURE_GATE_PROGRAM_ID,
    );
    assert_eq!(alias_address(&id), expected);
    assert_eq!(
        COOKIE_FEATURE_GATE_PROGRAM_ID.to_string(),
        "CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4"
    );
}

#[test]
fn nothing_changes_without_an_alias_account() {
    let id = subject();
    let (bank0, _forks) = bank_without(&id);
    assert_eq!(bank0.compute_pending_activation_slot(&id), None);
    let (bank1, _) = next_epoch(bank0);
    assert!(!bank1.feature_set.is_active(&id));
    assert!(bank1.get_account(alias_of(&id).unwrap()).is_none());
}

#[test]
fn alias_account_activates_the_upstream_feature() {
    let id = subject();
    let alias = *alias_of(&id).unwrap();
    let (bank0, _forks) = bank_without(&id);
    bank0.store_account(&alias, &pending_feature_account());

    let first_slot_next_epoch = bank0
        .epoch_schedule()
        .get_first_slot_in_epoch(bank0.epoch() + 1);
    assert_eq!(
        bank0.compute_pending_activation_slot(&id),
        Some(first_slot_next_epoch)
    );

    let (bank1, slot) = next_epoch(bank0);
    assert_eq!(slot, first_slot_next_epoch);
    assert!(bank1.feature_set.is_active(&id));
    assert_eq!(bank1.feature_set.activated_slot(&id), Some(slot));
    // Activation is recorded in the alias account; no upstream account appears.
    assert_eq!(activated_at(&bank1, &alias), Some(slot));
    assert!(bank1.get_account(&id).is_none());

    // It stays active in later epochs.
    let (bank2, _) = next_epoch(Arc::new(bank1));
    assert!(bank2.feature_set.is_active(&id));
    assert_eq!(bank2.feature_set.activated_slot(&id), Some(slot));
}

#[test]
fn alias_must_be_a_feature_account() {
    let id = subject();
    let alias = *alias_of(&id).unwrap();
    let (bank0, _forks) = bank_without(&id);
    // Anyone can send lamports to the PDA address; that must not count.
    bank0.store_account(
        &alias,
        &AccountSharedData::new(1_000_000_000, 9, &solana_sdk_ids::system_program::id()),
    );
    let (bank1, _) = next_epoch(bank0);
    assert!(!bank1.feature_set.is_active(&id));
    assert_eq!(
        bank1.get_account(&alias).unwrap().owner(),
        &solana_sdk_ids::system_program::id()
    );
}

#[test]
fn upstream_feature_account_takes_precedence() {
    let id = subject();
    let alias = *alias_of(&id).unwrap();
    let (bank0, _forks) = bank_without(&id);
    bank0.store_account(&id, &pending_feature_account());
    bank0.store_account(&alias, &pending_feature_account());

    let (bank1, slot) = next_epoch(bank0);
    assert!(bank1.feature_set.is_active(&id));
    assert_eq!(activated_at(&bank1, &id), Some(slot));
    // Stock behaviour: only the upstream account is updated.
    assert_eq!(activated_at(&bank1, &alias), None);
}

#[test]
fn denylisted_features_cannot_be_activated_by_alias() {
    for id in DENYLIST.iter() {
        let (bank0, _forks) = bank_without(id);
        // Even a valid feature account at the would-be PDA is ignored.
        bank0.store_account(&alias_address(id), &pending_feature_account());
        let (bank1, _) = next_epoch(bank0);
        assert!(!bank1.feature_set.is_active(id), "{id} activated");
    }
}

/// A plain account at an upstream feature address. Anyone can create one by
/// sending lamports to the address.
fn stray_account() -> AccountSharedData {
    AccountSharedData::new(1_000_000, 0, &solana_sdk_ids::system_program::id())
}

#[test]
fn lamports_at_upstream_address_do_not_block_the_alias() {
    let id = subject();
    let alias = *alias_of(&id).unwrap();
    let (bank0, _forks) = bank_without(&id);
    bank0.store_account(&id, &stray_account());
    bank0.store_account(&alias, &pending_feature_account());

    let first_slot_next_epoch = bank0
        .epoch_schedule()
        .get_first_slot_in_epoch(bank0.epoch() + 1);
    assert_eq!(
        bank0.compute_pending_activation_slot(&id),
        Some(first_slot_next_epoch)
    );

    let (bank1, slot) = next_epoch(bank0);
    assert!(bank1.feature_set.is_active(&id));
    assert_eq!(bank1.feature_set.activated_slot(&id), Some(slot));
    assert_eq!(activated_at(&bank1, &alias), Some(slot));
    // The plain account is left as it was.
    assert_eq!(bank1.get_account(&id), Some(stray_account()));
}

#[test]
fn lamports_at_upstream_address_without_alias_behave_as_stock() {
    let id = subject();
    let alias = *alias_of(&id).unwrap();
    let (bank0, _forks) = bank_without(&id);
    bank0.store_account(&id, &stray_account());
    bank0.store_account(&alias, &stray_account());
    // As on stock Agave, the plain upstream account is what callers see, so
    // the ledger tool still reports that it is not a feature account.
    assert_eq!(
        bank0.cookie_feature_account(&id),
        Some((id, stray_account()))
    );
    let (bank1, _) = next_epoch(bank0);
    assert!(!bank1.feature_set.is_active(&id));
}

/// A validator that restarts from a snapshot rebuilds its feature set from
/// accounts. A feature already activated through its alias must stay active
/// at its original slot even if someone holding the feature's own key later
/// creates a pending account at the upstream address. Otherwise a restarted
/// validator would disagree with one that kept running.
#[test]
fn activated_alias_survives_a_restart_with_a_pending_upstream_account() {
    let id = subject();
    let alias = *alias_of(&id).unwrap();
    let mut info = create_genesis_config(1_000_000_000_000);
    info.genesis_config.accounts.remove(&id);
    let (bank0, _forks) =
        Bank::new_for_tests(&info.genesis_config).wrap_with_bank_forks_for_tests();
    bank0.store_account_and_update_capitalization(&alias, &pending_feature_account());
    let (bank1, slot) = next_epoch(bank0);
    assert_eq!(bank1.feature_set.activated_slot(&id), Some(slot));

    bank1.store_account_and_update_capitalization(&id, &pending_feature_account());
    bank1.fill_bank_with_ticks_for_tests();
    bank1.set_block_id(Some(Hash::default()));

    let (_accounts_tmp, accounts_dir) = create_tmp_accounts_dir_for_tests();
    let bank_snapshots_dir = TempDir::new().unwrap();
    let full_snapshot_archives_dir = TempDir::new().unwrap();
    let incremental_snapshot_archives_dir = TempDir::new().unwrap();
    let snapshot_config = SnapshotConfig {
        full_snapshot_archives_dir: full_snapshot_archives_dir.path().to_path_buf(),
        incremental_snapshot_archives_dir: incremental_snapshot_archives_dir.path().to_path_buf(),
        bank_snapshots_dir: bank_snapshots_dir.path().to_path_buf(),
        ..SnapshotConfig::default()
    };
    let archive = bank_to_full_snapshot_archive(&snapshot_config, &bank1).unwrap();
    let restarted = bank_from_snapshot_archives(
        &[accounts_dir],
        &archive,
        None,
        &snapshot_config,
        &info.genesis_config,
        &RuntimeConfig::default(),
        None,
        Some(*bank1.leader()),
        None,
        false,
        false,
        false,
        ACCOUNTS_DB_CONFIG_FOR_TESTING,
        None,
        Arc::default(),
    )
    .unwrap();

    assert_eq!(restarted.feature_set.activated_slot(&id), Some(slot));
    assert_eq!(activated_at(&restarted, &alias), Some(slot));
}
