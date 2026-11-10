//! The release name operators see. Gossip still carries version 4.1.2.

/// Shown by `solana --version`, `agave-validator --version`, and `getVersion`.
pub const RELEASE: &str = "4.1.2-cookie.1";

/// Same shape as [`crate::Version::as_detailed_string`], with [`RELEASE`] in front.
pub fn detailed(version: &crate::Version) -> String {
    format!(
        "{RELEASE} (src:{:08x}; feat:{:08x}, client:{:?})",
        version.commit(),
        version.feature_set(),
        version.client(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_names_this_release() {
        let text = detailed(&crate::Version::default());
        assert!(text.starts_with("4.1.2-cookie.1 (src:"), "{text}");
        assert!(text.contains(", client:"), "{text}");
    }
}
