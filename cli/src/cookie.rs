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

/// `solana feature revoke` withdraws a pending feature by signing as the
/// feature's own account. A feature requested through the Cookie switch lives
/// at its alias, which only the `cookie-feature-gate` program can sign for, so
/// it can only be withdrawn by the switch's Revoke instruction, sent by the
/// switch authority (the Cookie multisig). Stops with that explanation instead
/// of the stock "has not been submitted" message.
pub(crate) async fn check_revoke_target(
    rpc_client: &RpcClient,
    feature_id: &Pubkey,
) -> Result<(), Box<dyn std::error::Error>> {
    let commitment = rpc_client.commitment();
    let upstream = rpc_client
        .get_account_with_commitment(feature_id, commitment)
        .await?
        .value;
    let alias_account = match alias_of(feature_id) {
        Some(alias) if !upstream.as_ref().is_some_and(is_feature_account) => {
            rpc_client
                .get_account_with_commitment(alias, commitment)
                .await?
                .value
        }
        _ => None,
    };
    match revoke_refusal(feature_id, upstream.as_ref(), alias_account.as_ref()) {
        Some(reason) => Err(reason.into()),
        None => Ok(()),
    }
}

/// Why `solana feature revoke` cannot handle `feature_id`, if it cannot.
fn revoke_refusal(
    feature_id: &Pubkey,
    upstream: Option<&Account>,
    alias_account: Option<&Account>,
) -> Option<String> {
    if upstream.is_some_and(is_feature_account) {
        return None;
    }
    let alias = alias_of(feature_id)?;
    match from_account(alias_account?)?.activated_at {
        None => Some(format!(
            "{feature_id} was requested through the Cookie feature switch (alias {alias}). \
             `solana feature revoke` cannot withdraw it, because only the cookie-feature-gate \
             program can sign for the alias. Withdraw it with a Revoke instruction from the \
             switch authority, the Cookie multisig."
        )),
        Some(_) => Some(format!("{feature_id} has already been fully activated")),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_feature_gate_interface::{Feature, create_account},
    };

    fn feature_account(activated_at: Option<u64>) -> Account {
        create_account(&Feature { activated_at }, 1_000_000).into()
    }

    fn feature_with_alias() -> Pubkey {
        agave_feature_set::block_revenue_sharing::id()
    }

    #[test]
    fn stock_revoke_handles_an_upstream_feature_account() {
        let id = feature_with_alias();
        let pending = feature_account(None);
        assert_eq!(revoke_refusal(&id, Some(&pending), None), None);
        // Even if an alias exists too, the upstream account wins, as in the runtime.
        assert_eq!(revoke_refusal(&id, Some(&pending), Some(&pending)), None);
    }

    #[test]
    fn pending_alias_points_to_the_multisig() {
        let id = feature_with_alias();
        let reason = revoke_refusal(&id, None, Some(&feature_account(None))).unwrap();
        assert!(reason.contains("Cookie feature switch"));
        assert!(reason.contains(&alias_of(&id).unwrap().to_string()));
        // A plain account at the upstream address does not change that.
        let stray = Account::new(1_000_000, 0, &Pubkey::default());
        assert!(revoke_refusal(&id, Some(&stray), Some(&feature_account(None))).is_some());
    }

    #[test]
    fn active_alias_is_reported_as_active() {
        let id = feature_with_alias();
        assert_eq!(
            revoke_refusal(&id, None, Some(&feature_account(Some(42)))),
            Some(format!("{id} has already been fully activated"))
        );
    }

    #[test]
    fn nothing_requested_leaves_the_stock_message() {
        let id = feature_with_alias();
        assert_eq!(revoke_refusal(&id, None, None), None);
        let not_a_feature = Account::new(1_000_000, 0, &Pubkey::default());
        assert_eq!(revoke_refusal(&id, None, Some(&not_a_feature)), None);
        // Features without an alias (the denylist) are left to the stock path.
        let denied = agave_feature_set::alpenglow::id();
        assert_eq!(
            revoke_refusal(&denied, None, Some(&feature_account(None))),
            None
        );
    }
}
