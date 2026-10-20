//! Cookie: `--clone-feature-set` must also see features activated through the
//! Cookie feature switches (see `agave_feature_set::cookie`), so a local test
//! validator cloned from Cookie Chain gets exactly Cookie's feature set.

use {
    agave_feature_set::cookie::alias_of, solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    solana_rpc_client_api::request::MAX_MULTIPLE_ACCOUNTS, std::collections::HashSet,
};

/// Removes from `deactivate` every feature whose Cookie alias is active on
/// the cluster behind `rpc_client`.
pub(crate) fn keep_alias_activated_features(
    deactivate: &mut HashSet<Pubkey>,
    rpc_client: &RpcClient,
) -> Result<(), String> {
    let candidates: Vec<(Pubkey, Pubkey)> = deactivate
        .iter()
        .filter_map(|id| alias_of(id).map(|alias| (*id, *alias)))
        .collect();
    for chunk in candidates.chunks(MAX_MULTIPLE_ACCOUNTS) {
        let aliases: Vec<Pubkey> = chunk.iter().map(|(_, alias)| *alias).collect();
        let accounts = rpc_client
            .get_multiple_accounts(&aliases)
            .map_err(|err| format!("Failed to fetch: {err}"))?;
        for ((feature_id, _), account) in chunk.iter().zip(accounts) {
            let active = account
                .as_ref()
                .and_then(solana_feature_gate_interface::from_account)
                .and_then(|feature| feature.activated_at)
                .is_some();
            if active {
                deactivate.remove(feature_id);
            }
        }
    }
    Ok(())
}
