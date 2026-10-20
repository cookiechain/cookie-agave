//! Config account layout: `[version: u8 = 1][authority: 32 bytes]`.

use solana_program::{program_error::ProgramError, pubkey::Pubkey};

pub const CONFIG_VERSION: u8 = 1;
pub const CONFIG_SIZE: usize = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub authority: Pubkey,
}

impl Config {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() != CONFIG_SIZE || data[0] != CONFIG_VERSION {
            return Err(ProgramError::UninitializedAccount);
        }
        let authority =
            Pubkey::try_from(&data[1..33]).map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(Self { authority })
    }

    pub fn pack(&self, data: &mut [u8]) -> Result<(), ProgramError> {
        if data.len() != CONFIG_SIZE {
            return Err(ProgramError::InvalidAccountData);
        }
        data[0] = CONFIG_VERSION;
        data[1..33].copy_from_slice(self.authority.as_ref());
        Ok(())
    }
}

/// Upgrade authority recorded in an upgradeable-loader ProgramData account.
///
/// Layout: `[u32 tag = 3][u64 slot][u8 option][32-byte authority]`.
pub fn programdata_upgrade_authority(data: &[u8]) -> Result<Option<Pubkey>, ProgramError> {
    if data.len() < 45 || data[0..4] != 3u32.to_le_bytes() {
        return Err(ProgramError::InvalidAccountData);
    }
    match data[12] {
        0 => Ok(None),
        1 => Ok(Some(
            Pubkey::try_from(&data[13..45]).map_err(|_| ProgramError::InvalidAccountData)?,
        )),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let config = Config {
            authority: Pubkey::new_unique(),
        };
        let mut data = [0u8; CONFIG_SIZE];
        config.pack(&mut data).unwrap();
        assert_eq!(Config::unpack(&data).unwrap(), config);
    }

    #[test]
    fn uninitialized_config_is_rejected() {
        assert_eq!(
            Config::unpack(&[0u8; CONFIG_SIZE]),
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(Config::unpack(&[]), Err(ProgramError::UninitializedAccount));
    }

    #[test]
    fn programdata_authority_parsing() {
        let authority = Pubkey::new_unique();
        let mut data = vec![0u8; 45 + 100];
        data[0..4].copy_from_slice(&3u32.to_le_bytes());
        data[4..12].copy_from_slice(&123u64.to_le_bytes());
        data[12] = 1;
        data[13..45].copy_from_slice(authority.as_ref());
        assert_eq!(
            programdata_upgrade_authority(&data).unwrap(),
            Some(authority)
        );
        data[12] = 0;
        assert_eq!(programdata_upgrade_authority(&data).unwrap(), None);
        data[0] = 2; // a Program account, not ProgramData
        assert!(programdata_upgrade_authority(&data).is_err());
    }
}
