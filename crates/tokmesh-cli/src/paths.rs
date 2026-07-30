//! CLI-side path helpers.
//!
//! The cross-platform config and cache directory resolution lives in
//! `tokmesh_core::paths` so the core crate's caches can resolve the same
//! locations without depending on tokmesh-cli. This module re-exports
//! those helpers for CLI callers.

pub use tokmesh_core::paths::{get_cache_dir, get_config_dir};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn re_exports_compile_and_match_core() {
        let _config: PathBuf = get_config_dir();
        let _cache: PathBuf = get_cache_dir();
    }
}
