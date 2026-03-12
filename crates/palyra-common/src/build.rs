use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BuildMetadata {
    pub version: &'static str,
    pub git_hash: &'static str,
    pub build_profile: &'static str,
}

#[must_use]
pub fn build_metadata() -> BuildMetadata {
    BuildMetadata {
        version: env!("CARGO_PKG_VERSION"),
        git_hash: option_env!("PALYRA_GIT_HASH").unwrap_or("unknown"),
        build_profile: if cfg!(debug_assertions) { "debug" } else { "release" },
    }
}
