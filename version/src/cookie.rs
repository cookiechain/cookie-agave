//! Cookie release numbers on the gossip version.
//!
//! Agave packs a prerelease into two bits of the minor version: stable, rc,
//! beta, or alpha. Those bits are full, and any prerelease forces the patch
//! to 0, so `4.1.2-cookie.1` cannot travel as a normal prerelease.
//!
//! A Cookie release stays a stable `major.minor.patch` on the wire. The
//! `cookie.N` number is written into the client id, above the ids Agave uses
//! (`0..=7`). A stock 4.1.2 node still deserializes the contact info and
//! shows the stable version. A node on this release reads the client id back
//! and shows `<upstream>-cookie.N`.

use crate::{ClientId, Prerelease, Version};

pub(crate) const IDENTIFIER: &str = "cookie";

/// `cookie.N` is client id `WIRE_BASE + N`.
/// `N` is 1..=255, clear of Agave's client ids (`0..=7`) and of `u16::MAX`,
/// which the version codec uses as an unknown client.
const WIRE_BASE: u16 = 0xC000;
const MAX_RELEASE: u16 = 0xFF;

pub(crate) fn wire_client(release: u16) -> Result<u16, &'static str> {
    if (1..=MAX_RELEASE).contains(&release) {
        Ok(WIRE_BASE + release)
    } else {
        Err("cookie release does not fit in the gossip client id")
    }
}

fn release_from_wire(wire: u16) -> Option<u16> {
    let release = wire.checked_sub(WIRE_BASE)?;
    (1..=MAX_RELEASE).contains(&release).then_some(release)
}

pub(crate) fn prerelease_from_wire(wire: u16, prerelease: Prerelease) -> Prerelease {
    if prerelease == Prerelease::Stable {
        if let Some(release) = release_from_wire(wire) {
            return Prerelease::Cookie(release);
        }
    }
    prerelease
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    #[test]
    fn cargo_version_is_what_gossip_shows() {
        let version = Version::default();
        assert_eq!(version.to_string(), env!("CARGO_PKG_VERSION"));
        let bytes = bincode::serialize(&version).unwrap();
        let decoded: Version = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded, version);
    }

    #[test]
    fn cookie_release_roundtrips_and_keeps_the_upstream_patch() {
        let version =
            Version::new_from_parts(4, 1, 2, 0, 0, ClientId::Agave, Prerelease::Cookie(1));
        assert_eq!(version.to_string(), "4.1.2-cookie.1");
        assert_eq!(
            version.as_detailed_string(),
            "4.1.2-cookie.1 (src:00000000; feat:00000000, client:Agave)",
        );

        let stable = Version::new_from_parts(4, 1, 2, 0, 0, ClientId::Agave, Prerelease::Stable);
        let cookie_bytes = bincode::serialize(&version).unwrap();
        let stable_bytes = bincode::serialize(&stable).unwrap();
        // Same major, minor, patch, commit, and feature set as stable 4.1.2.
        // Only the trailing client id differs, which is where `cookie.1` lives.
        assert_eq!(
            &cookie_bytes[..cookie_bytes.len() - 3],
            &stable_bytes[..stable_bytes.len() - 1],
        );

        let decoded: Version = bincode::deserialize(&cookie_bytes).unwrap();
        assert_eq!(decoded, version);
        assert_eq!(decoded.prerelease(), &Prerelease::Cookie(1));
        assert_eq!(decoded.patch(), 2);
        assert_eq!(decoded.client(), &ClientId::Agave);
    }

    #[test]
    fn cookie_prerelease_parses_when_it_fits_the_client_id() {
        assert_eq!(Prerelease::from_str("cookie.1"), Ok(Prerelease::Cookie(1)));
        assert_eq!(
            Prerelease::from_str("cookie.0"),
            Err(crate::ParsePrereleaseError::NumericPartNotAU16)
        );
        assert_eq!(
            Prerelease::from_str("cookie.256"),
            Err(crate::ParsePrereleaseError::NumericPartNotAU16)
        );
    }
}
