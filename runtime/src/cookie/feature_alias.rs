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
    /// A feature account at the upstream address wins (stock behaviour).
    /// Otherwise the Cookie alias is used if it is a feature account. Anyone
    /// can send lamports to an upstream feature address, so a plain account
    /// there does not hide the alias. Failing both, whatever is at the
    /// upstream address is returned, as on stock Agave. With no alias
    /// accounts on chain this is identical to stock Agave.
    pub fn cookie_feature_account(
        &self,
        feature_id: &Pubkey,
    ) -> Option<(Pubkey, AccountSharedData)> {
        let upstream = self.get_account_with_fixed_root(feature_id);
        if upstream.as_ref().is_some_and(is_feature_account) {
            return upstream.map(|account| (*feature_id, account));
        }
        alias_of(feature_id)
            .and_then(|alias| Some((*alias, self.get_account_with_fixed_root(alias)?)))
            .filter(|(_, account)| is_feature_account(account))
            .or_else(|| upstream.map(|account| (*feature_id, account)))
    }
}

fn is_feature_account(account: &AccountSharedData) -> bool {
    feature::from_account(account).is_some()
}
