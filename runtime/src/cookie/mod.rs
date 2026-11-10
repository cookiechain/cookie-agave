//! Cookie Chain extensions to the runtime.
//!
//! Everything Cookie-specific lives under this module. Upstream files only
//! call into it through small hooks, listed in `COOKIE-TOUCHPOINTS.md`.
//! See `COOKIE-RULES.md` before changing anything here.

pub mod feature_alias;

#[cfg(test)]
mod tests;
