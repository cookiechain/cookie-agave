//! Cookie: feature accounts as the CLI sees them.
//!
//! Features activated through the Cookie feature switches live at a Cookie
//! alias address (see `agave_feature_set::cookie`). These methods have the
//! same contract as the `RpcClient` calls they stand in for, but fall back to
//! the alias when a feature's own address holds no feature account. That keeps
//! `solana feature status`, `feature activate` and every command that checks
//! feature status behaving exactly as on Solana.
//!
//! Upstream call sites only swap the method name, for example
//! `.get_account(feature_id)` becomes `.get_feature_account(feature_id)`.

use {
    agave_feature_set::cookie::{alias_of, fill_from_aliases, missing_aliases},
    solana_account::Account,
    solana_commitment_config::CommitmentConfig,
    solana_feature_gate_interface::from_account,
    solana_pubkey::Pubkey,
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_rpc_client_api::{client_error::Result as ClientResult, response::RpcResult},
};

pub(crate) trait CookieFeatureAccounts {
    /// Like [`RpcClient::get_multiple_accounts`], for feature accounts.
    async fn get_feature_accounts(
        &self,
        feature_ids: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>>;

    /// Like [`RpcClient::get_account`], for a feature account. Returns the
    /// same error as `get_account` when neither the feature nor its alias
    /// exists.
    async fn get_feature_account(&self, feature_id: &Pubkey) -> ClientResult<Account>;

    /// Like [`RpcClient::get_account_with_commitment`], for a feature account.
    async fn get_feature_account_with_commitment(
        &self,
        feature_id: &Pubkey,
        commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>>;
}

impl CookieFeatureAccounts for RpcClient {
    async fn get_feature_accounts(
        &self,
        feature_ids: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>> {
        let mut accounts = self.get_multiple_accounts(feature_ids).await?;
        let (positions, aliases) = missing_aliases(feature_ids, &accounts, is_feature_account);
        if !aliases.is_empty() {
            let alias_accounts = self.get_multiple_accounts(&aliases).await?;
            fill_from_aliases(
                &mut accounts,
                &positions,
                alias_accounts,
                is_feature_account,
            );
        }
        Ok(accounts)
    }

    async fn get_feature_account(&self, feature_id: &Pubkey) -> ClientResult<Account> {
        match self
            .get_feature_accounts(&[*feature_id])
            .await?
            .pop()
            .flatten()
        {
            Some(account) => Ok(account),
            None => self.get_account(feature_id).await,
        }
    }

    async fn get_feature_account_with_commitment(
        &self,
        feature_id: &Pubkey,
        commitment: CommitmentConfig,
    ) -> RpcResult<Option<Account>> {
        let mut response = self
            .get_account_with_commitment(feature_id, commitment)
            .await?;
        if !response.value.as_ref().is_some_and(is_feature_account) {
            if let Some(alias) = alias_of(feature_id) {
                let alias_account = self
                    .get_account_with_commitment(alias, commitment)
                    .await?
                    .value
                    .filter(is_feature_account);
                if alias_account.is_some() {
                    response.value = alias_account;
                }
            }
        }
        Ok(response)
    }
}

fn is_feature_account(account: &Account) -> bool {
    from_account(account).is_some()
}
