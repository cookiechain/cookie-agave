//! Upstream feature IDs the switch refuses to activate: the Alpenglow
//! placeholder in Agave 4.1, and three IDs upstream abandoned.
//!
//! The validator ignores aliases for these anyway. Refusing them here means a
//! mistaken request cannot sit pending and switch on by itself if a later
//! release drops an ID from the validator's list.
//!
//! The program is built separately from the validator, so it keeps its own
//! copy. It must match `DENYLIST` in `feature-set/src/cookie.rs`; a test there
//! fails if the two lists differ.

use solana_program::pubkey::Pubkey;

pub const DENYLIST: [Pubkey; 4] = [
    // alpenglow (placeholder in Agave 4.1)
    solana_program::pubkey!("mustRekeyVm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP"),
    // enable_loader_v4
    solana_program::pubkey!("LoaderV4WasAbandoned11111111111111111111111"),
    // enable_extend_program_checked
    solana_program::pubkey!("ExtendProgCheckedWi11BeDe1eted11111111111111"),
    // stake_minimum_delegation_for_rewards
    solana_program::pubkey!("MinimumDe1egat1onForRewardsWi11BeDe1eted111"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_entries_are_distinct() {
        for (i, a) in DENYLIST.iter().enumerate() {
            for b in &DENYLIST[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
