//! Cookie Chain program swaps.
//!
//! Cookie's SPL Token and Stake programs are Loader v2 (non-upgradeable)
//! programs, and the upstream swaps for them either already ran with nothing
//! to copy (p-token, active since Cookie genesis), were deleted after mainnet
//! used them (SIMD-0196), or read from a buffer only upstream controls
//! (SIMD-0490). Each swap here reads a Cookie-controlled buffer and only
//! proceeds when the buffer holds the exact expected program.
//!
//! A swap runs once, in the epoch its feature activates. If it fails, the
//! feature stays active and the swap never runs again.

use {
    super::error::CoreBpfMigrationError, crate::bank::Bank, agave_feature_set as feature_set,
    ahash::AHashSet, log::error, solana_hash::Hash, solana_pubkey::Pubkey,
};

/// Buffer addresses are consensus constants: every validator must run a build
/// with the same three. They are the keypairs created offline in plan step 0.2
/// (10 Nov 2026); the buffers are written on chain at step 0.7.
pub mod token_buffer {
    solana_pubkey::declare_id!("7anfKyfCz8krv8Jvmg9DLAjS9WvzfDY2XfBJZyLyoYBF");
}

pub mod stake_buffer {
    solana_pubkey::declare_id!("djJonZTCuwfU9hkuntGjLZeXf6bziS76PuwDJUfxqHm");
}

pub mod stake_v5_buffer {
    solana_pubkey::declare_id!("BYeWRbFpEi8wf5UCXypQJ9CvHh3UrfggqF6MYy1UuLCx");
}

/// SHA-256 of solana-program/token p-token@v1.0.0 `spl_p_token.so`, trailing
/// zeros trimmed.
pub const P_TOKEN_1_0_0_HASH: Hash = Hash::new_from_array([
    67, 93, 241, 231, 15, 28, 166, 37, 142, 176, 26, 69, 7, 112, 22, 136, 199, 241, 162, 38, 86,
    227, 228, 239, 54, 29, 180, 115, 191, 71, 128, 117,
]);

/// SHA-256 of solana-program/stake program@v4.0.0 `solana_stake_program.so`,
/// trailing zeros trimmed.
pub const STAKE_4_0_0_HASH: Hash = Hash::new_from_array([
    110, 7, 50, 223, 105, 223, 72, 52, 114, 205, 59, 207, 246, 193, 28, 208, 197, 90, 189, 119,
    169, 43, 225, 190, 219, 225, 206, 50, 144, 108, 24, 184,
]);

/// SHA-256 of solana-program/stake program@v5.0.0 `solana_stake_program.so`,
/// trailing zeros trimmed.
pub const STAKE_5_0_0_HASH: Hash = Hash::new_from_array([
    237, 70, 152, 174, 50, 65, 80, 190, 64, 24, 48, 205, 48, 46, 212, 104, 36, 163, 214, 48, 247,
    86, 31, 93, 109, 13, 15, 139, 171, 40, 107, 204,
]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CookieSwapKind {
    /// Loader v2 program becomes a Loader v3 program with no upgrade authority.
    LoaderV2ToV3,
    /// Loader v3 program gets new program data; its upgrade authority is kept.
    CoreBpfUpgrade,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CookieProgramSwap {
    pub feature_id: Pubkey,
    pub program_id: Pubkey,
    pub source_buffer_address: Pubkey,
    pub verified_build_hash: Hash,
    pub kind: CookieSwapKind,
    pub datapoint_name: &'static str,
}

/// At most one swap per program runs at an epoch boundary: deploying the same
/// program twice in one slot trips the program cache ("Unexpected replacement
/// of an entry") and in release builds leaves the first deployment cached.
/// So if the stake migration and the v5 upgrade activate together, only the
/// migration runs and the v5 upgrade is spent without upgrading.
pub(crate) const COOKIE_PROGRAM_SWAPS: [CookieProgramSwap; 3] = [
    CookieProgramSwap {
        feature_id: feature_set::cookie::replace_spl_token_with_p_token::ID,
        program_id: feature_set::replace_spl_token_with_p_token::SPL_TOKEN_PROGRAM_ID,
        source_buffer_address: token_buffer::ID,
        verified_build_hash: P_TOKEN_1_0_0_HASH,
        kind: CookieSwapKind::LoaderV2ToV3,
        datapoint_name: "cookie_replace_spl_token_with_p_token",
    },
    CookieProgramSwap {
        feature_id: feature_set::migrate_stake_program_to_core_bpf::ID,
        program_id: solana_sdk_ids::stake::ID,
        source_buffer_address: stake_buffer::ID,
        verified_build_hash: STAKE_4_0_0_HASH,
        kind: CookieSwapKind::LoaderV2ToV3,
        datapoint_name: "cookie_migrate_stake_program_to_loader_v3",
    },
    CookieProgramSwap {
        feature_id: feature_set::upgrade_bpf_stake_program_to_v5::ID,
        program_id: solana_sdk_ids::stake::ID,
        source_buffer_address: stake_v5_buffer::ID,
        verified_build_hash: STAKE_5_0_0_HASH,
        kind: CookieSwapKind::CoreBpfUpgrade,
        datapoint_name: "cookie_upgrade_stake_program_to_v5",
    },
];

impl Bank {
    pub(crate) fn apply_cookie_program_swaps(
        &mut self,
        new_feature_activations: &AHashSet<Pubkey>,
    ) {
        let mut swapped_programs: Vec<Pubkey> = Vec::with_capacity(COOKIE_PROGRAM_SWAPS.len());
        for swap in COOKIE_PROGRAM_SWAPS.iter() {
            if !new_feature_activations.contains(&swap.feature_id) {
                continue;
            }
            if swapped_programs.contains(&swap.program_id) {
                error!(
                    "Skipping Cookie program swap '{}': {} was already swapped at this epoch \
                     boundary",
                    swap.datapoint_name, swap.program_id,
                );
                continue;
            }
            match self.apply_cookie_program_swap(swap) {
                Ok(()) => swapped_programs.push(swap.program_id),
                Err(e) => error!(
                    "Failed Cookie program swap '{}' for {} from buffer {}: {e}",
                    swap.datapoint_name, swap.program_id, swap.source_buffer_address,
                ),
            }
        }
    }

    pub(crate) fn apply_cookie_program_swap(
        &mut self,
        swap: &CookieProgramSwap,
    ) -> Result<(), CoreBpfMigrationError> {
        match swap.kind {
            CookieSwapKind::LoaderV2ToV3 => {
                let allow_prefunded = self
                    .feature_set
                    .snapshot()
                    .relax_programdata_account_check_migration;
                self.upgrade_loader_v2_program_with_loader_v3_program_checked(
                    &swap.program_id,
                    &swap.source_buffer_address,
                    allow_prefunded,
                    Some(swap.verified_build_hash),
                    swap.datapoint_name,
                )
            }
            CookieSwapKind::CoreBpfUpgrade => self.upgrade_core_bpf_program_checked(
                &swap.program_id,
                &swap.source_buffer_address,
                Some(swap.verified_build_hash),
                swap.datapoint_name,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::bank::{SlotLeader, test_utils::goto_end_of_slot, tests::create_genesis_config},
        agave_feature_set::FeatureSet,
        solana_account::{AccountSharedData, ReadableAccount, WritableAccount},
        solana_epoch_schedule::EpochSchedule,
        solana_feature_gate_interface::{self as feature, Feature},
        solana_instruction::Instruction,
        solana_keypair::Keypair,
        solana_loader_v3_interface::{get_program_data_address, state::UpgradeableLoaderState},
        solana_message::Message,
        solana_native_token::LAMPORTS_PER_SOL,
        solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable},
        solana_signer::Signer,
        solana_transaction::Transaction,
        std::sync::Arc,
    };

    const SLOTS_PER_EPOCH: u64 = 32;
    const P_TOKEN_SO: &str = "spl_p_token-1.0.0.so";
    const STAKE_4_SO: &str = "solana_stake_program-4.0.0.so";
    const STAKE_5_SO: &str = "solana_stake_program-5.0.0.so";

    fn program_file(name: &str) -> Vec<u8> {
        let path = format!("src/bank/builtins/core_bpf_migration/cookie_programs/{name}");
        std::fs::read(&path).unwrap_or_else(|err| {
            panic!("{path}: {err}. Download it with cookie-upgrade-plan's fetch_programs.sh")
        })
    }

    fn trimmed_hash(elf: &[u8]) -> Hash {
        let end = elf.iter().rposition(|&x| x != 0).map_or(0, |i| i + 1);
        solana_sha256_hasher::hash(&elf[..end])
    }

    /// Cookie's feature set before the upgrade, as far as these swaps care:
    /// SBPF v0 deployment and execution stay enabled.
    fn cookie_like_feature_set(inactive: &[Pubkey]) -> FeatureSet {
        let mut feature_set = FeatureSet::all_enabled();
        for id in [
            feature_set::disable_sbpf_v0_v1_v2_deployment::id(),
            feature_set::disable_sbpf_v0_execution::id(),
        ]
        .iter()
        .chain(inactive)
        {
            feature_set.deactivate(id);
        }
        feature_set
    }

    fn store_loader_v2_program(bank: &Bank, program_id: &Pubkey) {
        bank.store_account_and_update_capitalization(
            &get_program_data_address(program_id),
            &AccountSharedData::default(),
        );
        let elf = [4u8; 200];
        let lamports = bank.get_minimum_balance_for_rent_exemption(elf.len());
        let mut account = AccountSharedData::new(lamports, elf.len(), &bpf_loader::id());
        account.set_executable(true);
        account.data_as_mut_slice().copy_from_slice(&elf);
        bank.store_account_and_update_capitalization(program_id, &account);
    }

    fn store_buffer(bank: &Bank, address: &Pubkey, elf: &[u8], authority: Option<Pubkey>) {
        let metadata_size = UpgradeableLoaderState::size_of_buffer_metadata();
        let space = metadata_size + elf.len();
        let mut account = AccountSharedData::new_data_with_space(
            bank.get_minimum_balance_for_rent_exemption(space),
            &UpgradeableLoaderState::Buffer {
                authority_address: authority,
            },
            space,
            &bpf_loader_upgradeable::id(),
        )
        .unwrap();
        account.data_as_mut_slice()[metadata_size..].copy_from_slice(elf);
        bank.store_account_and_update_capitalization(address, &account);
    }

    fn assert_loader_v3_program(bank: &Bank, program_id: &Pubkey, elf: &[u8]) {
        let program = bank.get_account(program_id).unwrap();
        assert_eq!(program.owner(), &bpf_loader_upgradeable::id());
        let program_data = bank
            .get_account(&get_program_data_address(program_id))
            .unwrap();
        let metadata_size = UpgradeableLoaderState::size_of_programdata_metadata();
        match bincode::deserialize(&program_data.data()[..metadata_size]).unwrap() {
            UpgradeableLoaderState::ProgramData {
                upgrade_authority_address,
                ..
            } => assert_eq!(upgrade_authority_address, None),
            other => panic!("not program data: {other:?}"),
        }
        assert_eq!(&program_data.data()[metadata_size..], elf);
    }

    struct Chain {
        bank: Arc<Bank>,
        bank_forks: Arc<std::sync::RwLock<crate::bank_forks::BankForks>>,
        mint_keypair: Keypair,
    }

    impl Chain {
        fn genesis_bank(inactive: &[Pubkey]) -> (Bank, Keypair) {
            let (mut genesis_config, mint_keypair) =
                create_genesis_config(1_000_000 * LAMPORTS_PER_SOL);
            genesis_config.epoch_schedule =
                EpochSchedule::custom(SLOTS_PER_EPOCH, SLOTS_PER_EPOCH, false);
            let mut bank = Bank::new_for_tests(&genesis_config);
            bank.feature_set = Arc::new(cookie_like_feature_set(inactive));
            bank.apply_activated_features();
            (bank, mint_keypair)
        }

        fn start(bank: Bank, mint_keypair: Keypair) -> Self {
            let (bank, bank_forks) = bank.wrap_with_bank_forks_for_tests();
            Self {
                bank,
                bank_forks,
                mint_keypair,
            }
        }

        fn request_activation(&self, feature_id: &Pubkey) {
            self.bank.store_account_and_update_capitalization(
                feature_id,
                &feature::create_account(&Feature::default(), 42),
            );
        }

        fn advance_to(&mut self, slot: u64) {
            goto_end_of_slot(self.bank.clone());
            self.bank = Bank::new_from_parent_with_bank_forks(
                &self.bank_forks,
                self.bank.clone(),
                SlotLeader::default(),
                slot,
            );
        }

        fn next_epoch(&mut self) {
            let next = (self.bank.epoch() + 1) * SLOTS_PER_EPOCH;
            self.advance_to(next);
        }

        fn next_slot(&mut self) {
            let next = self.bank.slot() + 1;
            self.advance_to(next);
        }

        fn invoke(&self, program_id: &Pubkey, data: &[u8]) {
            self.bank
                .process_transaction(&Transaction::new(
                    &[&self.mint_keypair],
                    Message::new(
                        &[Instruction::new_with_bytes(*program_id, data, vec![])],
                        Some(&self.mint_keypair.pubkey()),
                    ),
                    self.bank.last_blockhash(),
                ))
                .unwrap();
        }
    }

    /// Bincode index of `StakeInstruction::GetMinimumDelegation`.
    const GET_MINIMUM_DELEGATION: [u8; 4] = [13, 0, 0, 0];

    #[test]
    fn cookie_swap_hashes_match_release_files() {
        assert_eq!(trimmed_hash(&program_file(P_TOKEN_SO)), P_TOKEN_1_0_0_HASH);
        assert_eq!(trimmed_hash(&program_file(STAKE_4_SO)), STAKE_4_0_0_HASH);
        assert_eq!(trimmed_hash(&program_file(STAKE_5_SO)), STAKE_5_0_0_HASH);
    }

    /// Run before tagging a release: `cargo test -p solana-runtime
    /// cookie_release_buffers_configured -- --ignored`.
    #[test]
    #[ignore]
    fn cookie_release_buffers_configured() {
        for swap in COOKIE_PROGRAM_SWAPS {
            assert!(
                !swap
                    .source_buffer_address
                    .to_string()
                    .contains("P1aceho1der"),
                "{} still uses a placeholder buffer address",
                swap.datapoint_name,
            );
        }
    }

    #[test]
    fn cookie_token_trigger_replaces_spl_token_with_p_token() {
        let trigger = feature_set::cookie::replace_spl_token_with_p_token::id();
        let token = feature_set::replace_spl_token_with_p_token::SPL_TOKEN_PROGRAM_ID;
        let elf = program_file(P_TOKEN_SO);

        let (bank, mint_keypair) = Chain::genesis_bank(&[trigger]);
        store_loader_v2_program(&bank, &token);
        store_buffer(&bank, &token_buffer::ID, &elf, Some(Pubkey::new_unique()));
        let mut chain = Chain::start(bank, mint_keypair);

        chain.next_epoch();
        assert!(!chain.bank.feature_set.is_active(&trigger));
        chain.request_activation(&trigger);
        let capitalization_before = chain.bank.capitalization();
        chain.next_epoch();

        assert!(chain.bank.feature_set.is_active(&trigger));
        assert_loader_v3_program(&chain.bank, &token, &elf);
        assert!(chain.bank.get_account(&token_buffer::ID).is_none());
        assert!(chain.bank.capitalization() < capitalization_before);
    }

    #[test]
    fn cookie_swap_refuses_unexpected_buffer_contents() {
        let trigger = feature_set::cookie::replace_spl_token_with_p_token::id();
        let token = feature_set::replace_spl_token_with_p_token::SPL_TOKEN_PROGRAM_ID;

        let (bank, mint_keypair) = Chain::genesis_bank(&[trigger]);
        store_loader_v2_program(&bank, &token);
        store_buffer(&bank, &token_buffer::ID, &program_file(STAKE_5_SO), None);
        let mut chain = Chain::start(bank, mint_keypair);

        chain.request_activation(&trigger);
        chain.next_epoch();

        assert!(chain.bank.feature_set.is_active(&trigger));
        assert_eq!(
            chain.bank.get_account(&token).unwrap().owner(),
            &bpf_loader::id()
        );
        assert!(chain.bank.get_account(&token_buffer::ID).is_some());
    }

    #[test]
    fn cookie_stake_moves_to_loader_v3_then_upgrades_to_v5() {
        let migrate = feature_set::migrate_stake_program_to_core_bpf::id();
        let v5 = feature_set::upgrade_bpf_stake_program_to_v5::id();
        let stake = solana_sdk_ids::stake::id();
        let stake_4 = program_file(STAKE_4_SO);
        let stake_5 = program_file(STAKE_5_SO);
        let vault = Some(Pubkey::new_unique());

        let (bank, mint_keypair) = Chain::genesis_bank(&[migrate, v5]);
        store_loader_v2_program(&bank, &stake);
        store_buffer(&bank, &stake_buffer::ID, &stake_4, vault);
        store_buffer(&bank, &stake_v5_buffer::ID, &stake_5, vault);
        let mut chain = Chain::start(bank, mint_keypair);

        chain.request_activation(&migrate);
        chain.next_epoch();
        assert_loader_v3_program(&chain.bank, &stake, &stake_4);
        assert!(chain.bank.get_account(&stake_buffer::ID).is_none());
        assert!(chain.bank.get_account(&stake_v5_buffer::ID).is_some());
        chain.next_slot();
        chain.invoke(&stake, &GET_MINIMUM_DELEGATION);

        chain.request_activation(&v5);
        chain.next_epoch();
        assert!(chain.bank.feature_set.is_active(&v5));
        assert_loader_v3_program(&chain.bank, &stake, &stake_5);
        assert!(chain.bank.get_account(&stake_v5_buffer::ID).is_none());
        assert_eq!(
            crate::stake_utils::get_minimum_delegation(
                chain
                    .bank
                    .feature_set
                    .snapshot()
                    .upgrade_bpf_stake_program_to_v5
            ),
            LAMPORTS_PER_SOL
        );
        chain.next_slot();
        chain.invoke(&stake, &GET_MINIMUM_DELEGATION);
    }

    #[test]
    fn cookie_stake_migration_and_v5_in_same_epoch_only_migrates() {
        let migrate = feature_set::migrate_stake_program_to_core_bpf::id();
        let v5 = feature_set::upgrade_bpf_stake_program_to_v5::id();
        let stake = solana_sdk_ids::stake::id();
        let stake_4 = program_file(STAKE_4_SO);

        let (bank, mint_keypair) = Chain::genesis_bank(&[migrate, v5]);
        store_loader_v2_program(&bank, &stake);
        store_buffer(&bank, &stake_buffer::ID, &stake_4, None);
        store_buffer(&bank, &stake_v5_buffer::ID, &program_file(STAKE_5_SO), None);
        let mut chain = Chain::start(bank, mint_keypair);

        chain.request_activation(&migrate);
        chain.request_activation(&v5);
        chain.next_epoch();
        assert!(chain.bank.feature_set.is_active(&v5));
        assert_loader_v3_program(&chain.bank, &stake, &stake_4);
        assert!(chain.bank.get_account(&stake_v5_buffer::ID).is_some());
        chain.next_slot();
        chain.invoke(&stake, &GET_MINIMUM_DELEGATION);
    }

    #[test]
    fn cookie_v5_without_loader_v3_stake_leaves_program_but_raises_minimum() {
        let v5 = feature_set::upgrade_bpf_stake_program_to_v5::id();
        let stake = solana_sdk_ids::stake::id();

        let (bank, mint_keypair) = Chain::genesis_bank(&[v5]);
        store_loader_v2_program(&bank, &stake);
        store_buffer(&bank, &stake_v5_buffer::ID, &program_file(STAKE_5_SO), None);
        let mut chain = Chain::start(bank, mint_keypair);

        chain.request_activation(&v5);
        chain.next_epoch();
        assert!(
            chain
                .bank
                .feature_set
                .snapshot()
                .upgrade_bpf_stake_program_to_v5
        );
        assert_eq!(
            chain.bank.get_account(&stake).unwrap().owner(),
            &bpf_loader::id()
        );
        assert!(chain.bank.get_account(&stake_v5_buffer::ID).is_some());
    }
}
