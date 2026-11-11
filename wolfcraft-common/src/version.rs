//! Version numbers that must stay compatible across updates.

/// Format of a world folder (`world.toml`, regions, side files). Bump together with a migration
/// step in `WorldConfig::load`.
///  1 — first release.
///  2 — `format_version` field, `dimension.txt`, `commands.txt`, `hell/` regions.
pub const SAVE_FORMAT: u32 = 2;

/// Version of the modding API (WolfForge). Mods declare the version they were written for.
pub const MOD_API_VERSION: u32 = 1;

/// Can a mod written for API `wanted` run on this build?
pub fn mod_api_compatible(wanted: u32) -> bool {
    wanted >= 1 && wanted <= MOD_API_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_api_accepts_current_and_rejects_future() {
        assert!(mod_api_compatible(MOD_API_VERSION));
        assert!(!mod_api_compatible(MOD_API_VERSION + 1));
        assert!(!mod_api_compatible(0));
    }
}