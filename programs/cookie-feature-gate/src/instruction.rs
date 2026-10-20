//! Instruction encoding and builders.
//!
//! Wire format: one tag byte followed by one 32-byte public key.

use {
    crate::{
        BPF_LOADER_UPGRADEABLE_ID, FEATURE_PROGRAM_ID, INCINERATOR_ID, alias_address,
        config_address,
    },
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program_error::ProgramError,
        pubkey::Pubkey,
    },
    solana_system_interface::program as system_program,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateInstruction {
    /// Accounts: `[w] config, [w,s] payer, [s] upgrade authority, [] program data, [] system program`
    Initialize { authority: Pubkey },
    /// Accounts: `[w] config, [s] current authority`
    SetAuthority { new_authority: Pubkey },
    /// Accounts: `[] config, [s] authority, [w] alias, [w,s] payer, [] system program`
    Activate { feature_id: Pubkey },
    /// Accounts: `[] config, [s] authority, [w] alias, [w] incinerator, [] feature program, [] system program`
    Revoke { feature_id: Pubkey },
}

impl GateInstruction {
    pub fn pack(&self) -> Vec<u8> {
        let (tag, key) = match self {
            Self::Initialize { authority } => (0u8, authority),
            Self::SetAuthority { new_authority } => (1, new_authority),
            Self::Activate { feature_id } => (2, feature_id),
            Self::Revoke { feature_id } => (3, feature_id),
        };
        let mut data = Vec::with_capacity(33);
        data.push(tag);
        data.extend_from_slice(key.as_ref());
        data
    }

    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() != 33 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let key =
            Pubkey::try_from(&data[1..33]).map_err(|_| ProgramError::InvalidInstructionData)?;
        Ok(match data[0] {
            0 => Self::Initialize { authority: key },
            1 => Self::SetAuthority { new_authority: key },
            2 => Self::Activate { feature_id: key },
            3 => Self::Revoke { feature_id: key },
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}

/// ProgramData address of this program under the upgradeable loader.
pub fn program_data_address() -> Pubkey {
    Pubkey::find_program_address(&[crate::id().as_ref()], &BPF_LOADER_UPGRADEABLE_ID).0
}

pub fn initialize(payer: &Pubkey, upgrade_authority: &Pubkey, authority: &Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(config_address().0, false),
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*upgrade_authority, true),
            AccountMeta::new_readonly(program_data_address(), false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: GateInstruction::Initialize {
            authority: *authority,
        }
        .pack(),
    }
}

pub fn set_authority(authority: &Pubkey, new_authority: &Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(config_address().0, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data: GateInstruction::SetAuthority {
            new_authority: *new_authority,
        }
        .pack(),
    }
}

pub fn activate(authority: &Pubkey, payer: &Pubkey, feature_id: &Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(config_address().0, false),
            AccountMeta::new_readonly(*authority, true),
            AccountMeta::new(alias_address(feature_id).0, false),
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: GateInstruction::Activate {
            feature_id: *feature_id,
        }
        .pack(),
    }
}

pub fn revoke(authority: &Pubkey, feature_id: &Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(config_address().0, false),
            AccountMeta::new_readonly(*authority, true),
            AccountMeta::new(alias_address(feature_id).0, false),
            AccountMeta::new(INCINERATOR_ID, false),
            AccountMeta::new_readonly(FEATURE_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: GateInstruction::Revoke {
            feature_id: *feature_id,
        }
        .pack(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_round_trip() {
        let key = Pubkey::new_unique();
        for ix in [
            GateInstruction::Initialize { authority: key },
            GateInstruction::SetAuthority { new_authority: key },
            GateInstruction::Activate { feature_id: key },
            GateInstruction::Revoke { feature_id: key },
        ] {
            assert_eq!(GateInstruction::unpack(&ix.pack()).unwrap(), ix);
        }
    }

    #[test]
    fn malformed_data_is_rejected() {
        assert!(GateInstruction::unpack(&[]).is_err());
        assert!(GateInstruction::unpack(&[2u8; 32]).is_err());
        assert!(GateInstruction::unpack(&[9u8; 33]).is_err());
    }

    #[test]
    fn alias_seeds_match_the_validator() {
        // Must stay identical to runtime/src/cookie/feature_alias.rs.
        let feature_id = Pubkey::new_unique();
        let (expected, _) = Pubkey::find_program_address(
            &[b"cookie-alias", feature_id.as_ref()],
            &solana_program::pubkey!("CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4"),
        );
        assert_eq!(alias_address(&feature_id).0, expected);
    }
}
