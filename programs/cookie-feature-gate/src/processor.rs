//! Instruction processing.

use {
    crate::{
        ALIAS_SEED, BPF_LOADER_UPGRADEABLE_ID, CONFIG_SEED, FEATURE_ACCOUNT_SIZE,
        FEATURE_PROGRAM_ID, INCINERATOR_ID, alias_address, config_address,
        instruction::GateInstruction,
        state::{CONFIG_SIZE, Config, programdata_upgrade_authority},
    },
    solana_program::{
        account_info::{AccountInfo, next_account_info},
        entrypoint::ProgramResult,
        instruction::{AccountMeta, Instruction},
        msg,
        program::{invoke, invoke_signed},
        program_error::ProgramError,
        pubkey::Pubkey,
        rent::Rent,
        sysvar::Sysvar,
    },
    solana_system_interface::{instruction as system_instruction, program as system_program},
};

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    match GateInstruction::unpack(data)? {
        GateInstruction::Initialize { authority } => initialize(program_id, accounts, authority),
        GateInstruction::SetAuthority { new_authority } => {
            set_authority(program_id, accounts, new_authority)
        }
        GateInstruction::Activate { feature_id } => activate(program_id, accounts, feature_id),
        GateInstruction::Revoke { feature_id } => revoke(program_id, accounts, feature_id),
    }
}

fn require(condition: bool, error: ProgramError, why: &str) -> ProgramResult {
    if condition {
        Ok(())
    } else {
        msg!("cookie-feature-gate: {}", why);
        Err(error)
    }
}

/// Creates a PDA account owned by `owner`. Works even if someone has already
/// sent lamports to the address: tops it up to rent exemption, then
/// allocates and assigns it.
fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    space: u64,
    owner: &Pubkey,
    seeds: &[&[u8]],
) -> ProgramResult {
    let needed = Rent::get()?.minimum_balance(space as usize);
    let current = target.lamports();
    if current == 0 {
        return invoke_signed(
            &system_instruction::create_account(payer.key, target.key, needed, space, owner),
            &[payer.clone(), target.clone(), system.clone()],
            &[seeds],
        );
    }
    if current < needed {
        invoke(
            &system_instruction::transfer(payer.key, target.key, needed - current),
            &[payer.clone(), target.clone(), system.clone()],
        )?;
    }
    invoke_signed(
        &system_instruction::allocate(target.key, space),
        &[target.clone(), system.clone()],
        &[seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(target.key, owner),
        &[target.clone(), system.clone()],
        &[seeds],
    )
}

/// Loads the config and checks that `authority` is its authority and signed.
fn check_authority(
    program_id: &Pubkey,
    config: &AccountInfo,
    authority: &AccountInfo,
) -> ProgramResult {
    require(
        *config.key == config_address().0,
        ProgramError::InvalidArgument,
        "wrong config account",
    )?;
    require(
        config.owner == program_id,
        ProgramError::UninitializedAccount,
        "not initialized",
    )?;
    let state = Config::unpack(&config.data.borrow())?;
    require(
        *authority.key == state.authority,
        ProgramError::InvalidArgument,
        "wrong authority",
    )?;
    require(
        authority.is_signer,
        ProgramError::MissingRequiredSignature,
        "authority must sign",
    )
}

fn initialize(program_id: &Pubkey, accounts: &[AccountInfo], authority: Pubkey) -> ProgramResult {
    let it = &mut accounts.iter();
    let config = next_account_info(it)?;
    let payer = next_account_info(it)?;
    let upgrade_authority = next_account_info(it)?;
    let program_data = next_account_info(it)?;
    let system = next_account_info(it)?;

    let (config_key, bump) = config_address();
    require(
        *config.key == config_key,
        ProgramError::InvalidArgument,
        "wrong config account",
    )?;
    require(
        *system.key == system_program::ID,
        ProgramError::IncorrectProgramId,
        "wrong system program",
    )?;
    require(
        payer.is_signer,
        ProgramError::MissingRequiredSignature,
        "payer must sign",
    )?;
    require(
        upgrade_authority.is_signer,
        ProgramError::MissingRequiredSignature,
        "upgrade authority must sign",
    )?;

    let (program_data_key, _) =
        Pubkey::find_program_address(&[program_id.as_ref()], &BPF_LOADER_UPGRADEABLE_ID);
    require(
        *program_data.key == program_data_key,
        ProgramError::InvalidArgument,
        "wrong program data account",
    )?;
    require(
        *program_data.owner == BPF_LOADER_UPGRADEABLE_ID,
        ProgramError::IncorrectProgramId,
        "program data not owned by the upgradeable loader",
    )?;
    let recorded = programdata_upgrade_authority(&program_data.data.borrow())?;
    require(
        recorded == Some(*upgrade_authority.key),
        ProgramError::InvalidArgument,
        "signer is not the upgrade authority",
    )?;

    require(
        *config.owner == system_program::ID && config.data_is_empty(),
        ProgramError::AccountAlreadyInitialized,
        "already initialized",
    )?;
    create_pda_account(
        payer,
        config,
        system,
        CONFIG_SIZE as u64,
        program_id,
        &[CONFIG_SEED, &[bump]],
    )?;
    Config { authority }.pack(&mut config.data.borrow_mut())?;
    msg!("cookie-feature-gate: initialized, authority {}", authority);
    Ok(())
}

fn set_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_authority: Pubkey,
) -> ProgramResult {
    let it = &mut accounts.iter();
    let config = next_account_info(it)?;
    let authority = next_account_info(it)?;
    check_authority(program_id, config, authority)?;
    Config {
        authority: new_authority,
    }
    .pack(&mut config.data.borrow_mut())?;
    msg!(
        "cookie-feature-gate: authority changed to {}",
        new_authority
    );
    Ok(())
}

fn activate(program_id: &Pubkey, accounts: &[AccountInfo], feature_id: Pubkey) -> ProgramResult {
    let it = &mut accounts.iter();
    let config = next_account_info(it)?;
    let authority = next_account_info(it)?;
    let alias = next_account_info(it)?;
    let payer = next_account_info(it)?;
    let system = next_account_info(it)?;
    check_authority(program_id, config, authority)?;

    let (alias_key, bump) = alias_address(&feature_id);
    require(
        *alias.key == alias_key,
        ProgramError::InvalidArgument,
        "wrong alias account",
    )?;
    require(
        *system.key == system_program::ID,
        ProgramError::IncorrectProgramId,
        "wrong system program",
    )?;
    require(
        payer.is_signer,
        ProgramError::MissingRequiredSignature,
        "payer must sign",
    )?;
    require(
        *alias.owner != FEATURE_PROGRAM_ID,
        ProgramError::AccountAlreadyInitialized,
        "feature already requested or active",
    )?;
    require(
        *alias.owner == system_program::ID && alias.data_is_empty(),
        ProgramError::InvalidAccountData,
        "alias address is in an unexpected state",
    )?;

    // An all-zero 9-byte account owned by the feature program is a pending
    // feature: `Feature { activated_at: None }`.
    create_pda_account(
        payer,
        alias,
        system,
        FEATURE_ACCOUNT_SIZE,
        &FEATURE_PROGRAM_ID,
        &[ALIAS_SEED, feature_id.as_ref(), &[bump]],
    )?;
    msg!(
        "cookie-feature-gate: activation requested for {} (alias {})",
        feature_id,
        alias_key
    );
    Ok(())
}

fn revoke(program_id: &Pubkey, accounts: &[AccountInfo], feature_id: Pubkey) -> ProgramResult {
    let it = &mut accounts.iter();
    let config = next_account_info(it)?;
    let authority = next_account_info(it)?;
    let alias = next_account_info(it)?;
    let incinerator = next_account_info(it)?;
    let feature_program = next_account_info(it)?;
    let system = next_account_info(it)?;
    check_authority(program_id, config, authority)?;

    let (alias_key, bump) = alias_address(&feature_id);
    require(
        *alias.key == alias_key,
        ProgramError::InvalidArgument,
        "wrong alias account",
    )?;
    require(
        *incinerator.key == INCINERATOR_ID,
        ProgramError::InvalidArgument,
        "wrong incinerator",
    )?;
    require(
        *feature_program.key == FEATURE_PROGRAM_ID,
        ProgramError::IncorrectProgramId,
        "wrong feature program",
    )?;
    require(
        *system.key == system_program::ID,
        ProgramError::IncorrectProgramId,
        "wrong system program",
    )?;

    // The feature program only revokes pending activations; it rejects
    // features that are already active.
    let ix = Instruction {
        program_id: FEATURE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(alias_key, true),
            AccountMeta::new(INCINERATOR_ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: vec![0],
    };
    invoke_signed(
        &ix,
        &[
            alias.clone(),
            incinerator.clone(),
            system.clone(),
            feature_program.clone(),
        ],
        &[&[ALIAS_SEED, feature_id.as_ref(), &[bump]]],
    )?;
    msg!(
        "cookie-feature-gate: pending activation revoked for {}",
        feature_id
    );
    Ok(())
}
