//! Cookie feature aliases in the bank.
//!
//! The alias addresses are defined once in `agave_feature_set::cookie` and
//! shared with the client tools. This module only teaches the bank where to
//! find a feature's activation state.

pub use agave_feature_set::cookie::{
    ALIAS_SEED, COOKIE_FEATURE_GATE_PROGRAM_ID, DENYLIST, alias_address, alias_of,
};
use {
    crate::bank::Bank, solana_account::AccountSharedData, solana_feature_gate_interface as feature,
    solana_pubkey::Pubkey,
};

impl Bank {
    /// The account that holds activation state for `feature_id`, and its
    /// address.
    ///
    /// The upstream feature account wins if it exists (stock behaviour).
    /// Otherwise the Cookie alias is used, but only if it is a valid feature
    /// account. With no alias accounts on chain this is identical to stock
    /// Agave.
    pub fn cookie_feature_account(
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
