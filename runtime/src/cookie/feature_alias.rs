//! Cookie feature aliases in the bank.
//!
//! The alias addresses are defined once in `agave_feature_set::cookie` and
//! shared with the client tools. This module only teaches the bank where to
//! find a feature's activation state.

pub use agave_feature_set::cookie::{
    ALIAS_SEED, COOKIE_FEATURE_GATE_PROGRAM_ID, DENYLIST, alias_address, alias_of,
};
use {
    crate::bank::Bank,
    agave_feature_set::cookie::{FeatureRecord, alias_wins},
    solana_account::AccountSharedData,
    solana_feature_gate_interface as feature,
    solana_pubkey::Pubkey,
};

impl Bank {
    /// The account that holds activation state for `feature_id`, and its
    /// address.
    ///
    /// The upstream address and the alias are ranked by [`FeatureRecord`]: an
    /// activated feature account beats a pending one, which beats anything
    /// else, and on a tie the upstream address wins (stock behaviour). So
    /// lamports sent to the upstream address cannot hide the alias, and
    /// neither can a pending account someone later creates there with the
    /// feature's own key. The second matters on restart: the feature set is
    /// then rebuilt from accounts, and a restarted validator must agree with
    /// one that kept running. With nothing better at the alias, whatever is at
    /// the upstream address is returned, as on stock Agave. With no alias
    /// accounts on chain this is identical to stock Agave.
    pub fn cookie_feature_account(
        &self,
        feature_id: &Pubkey,
    ) -> Option<(Pubkey, AccountSharedData)> {
        let upstream = self.get_account_with_fixed_root(feature_id);
        let upstream_record = upstream.as_ref().map_or(FeatureRecord::NotAFeature, record);
        if upstream_record == FeatureRecord::Activated {
            return upstream.map(|account| (*feature_id, account));
        }
        match alias_of(feature_id)
            .and_then(|alias| Some((*alias, self.get_account_with_fixed_root(alias)?)))
        {
            Some((alias, account)) if alias_wins(upstream_record, record(&account)) => {
                Some((alias, account))
            }
            _ => upstream.map(|account| (*feature_id, account)),
        }
    }
}

fn record(account: &AccountSharedData) -> FeatureRecord {
    match feature::from_account(account) {
        None => FeatureRecord::NotAFeature,
        Some(feature) if feature.activated_at.is_some() => FeatureRecord::Activated,
        Some(_) => FeatureRecord::Pending,
    }
}
