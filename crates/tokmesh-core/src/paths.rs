//! Cross-platform resolution for tokmesh's user config and cache dirs.
//!
//! Tokmesh-core needs the same path helpers tokmesh-cli uses (settings
//! and message/pricing caches read from related directories), so the
//! resolver lives here and is re-exported from tokmesh-cli for callers
//! that already imported it from there. macOS users following the docs
//! expect `~/.config/tokmesh/` because that is what `auth.rs`,
//! `cursor.rs`, and `antigravity.rs` already write to.
//! `dirs::config_dir()` would instead return `~/Library/Application Support/`
//! on macOS, splitting state across two roots and silently ignoring
//! settings.json edits the user made via the documented path. This module
//! enforces the unified `~/.config/tokmesh/` location on macOS + Linux,
//! while keeping the platform default on Windows.

use std::path::PathBuf;

/// Resolve the tokmesh config dir, honoring `TOKMESH_CONFIG_DIR` first.
///
/// Resolution order:
/// 1. `TOKMESH_CONFIG_DIR` taken verbatim when set to a non-empty value.
///    Absolute paths are recommended; relative paths are accepted and
///    resolved against the process CWD. Empty strings are treated as
///    unset so the user gets the platform default instead of a surprise
///    `./` write.
/// 2. macOS: `$HOME/.config/tokmesh` (overrides `dirs::config_dir()`,
///    which would return `~/Library/Application Support/` and split state
///    across two roots — see module docs).
/// 3. Linux: `dirs::config_dir().join("tokmesh")` so XDG_CONFIG_HOME is
///    honored. Falls through to `$HOME/.config/tokmesh` when neither
///    `XDG_CONFIG_HOME` nor `HOME` resolve.
/// 4. Windows (and any other platform): `dirs::config_dir().join("tokmesh")`.
/// 5. Last-ditch fallback: `./.tokmesh` so a missing HOME never panics.
pub fn get_config_dir() -> PathBuf {
    if let Some(custom) = std::env::var_os("TOKMESH_CONFIG_DIR") {
        if !custom.is_empty() {
            return PathBuf::from(custom);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(".config").join("tokmesh");
        }
    }

    dirs::config_dir()
        .map(|d| d.join("tokmesh"))
        .unwrap_or_else(|| PathBuf::from(".tokmesh"))
}

/// Resolve the tokmesh cache dir as `<config_dir>/cache`.
///
/// Caches (TUI display data, source-message bincode, pricing JSON, the
/// OpenCode migration record, Wrapped fonts/images) all live under this
/// single subdirectory so an isolated profile (`TOKMESH_CONFIG_DIR=...`)
/// covers everything in one shot, and so `rm -rf <cache_dir>` is always
/// safe — no durable state mixed in.
pub fn get_cache_dir() -> PathBuf {
    get_config_dir().join("cache")
}

/// Whether `TOKMESH_CONFIG_DIR` is explicitly set in the environment.
pub fn is_config_dir_overridden() -> bool {
    std::env::var_os("TOKMESH_CONFIG_DIR").is_some_and(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::path::Path;

    fn save_env() -> (
        Option<std::ffi::OsString>,
        Option<std::ffi::OsString>,
        Option<std::ffi::OsString>,
    ) {
        (
            env::var_os("TOKMESH_CONFIG_DIR"),
            env::var_os("HOME"),
            env::var_os("XDG_CONFIG_HOME"),
        )
    }

    fn restore_env(
        prev: (
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
            Option<std::ffi::OsString>,
        ),
    ) {
        unsafe {
            match prev.0 {
                Some(v) => env::set_var("TOKMESH_CONFIG_DIR", v),
                None => env::remove_var("TOKMESH_CONFIG_DIR"),
            }
            match prev.1 {
                Some(v) => env::set_var("HOME", v),
                None => env::remove_var("HOME"),
            }
            match prev.2 {
                Some(v) => env::set_var("XDG_CONFIG_HOME", v),
                None => env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }

    #[test]
    #[serial]
    fn env_override_is_returned_verbatim() {
        let prev = save_env();
        unsafe {
            env::set_var("TOKMESH_CONFIG_DIR", "/tmp/tokmesh-custom");
        }
        assert_eq!(get_config_dir(), PathBuf::from("/tmp/tokmesh-custom"));
        restore_env(prev);
    }

    #[test]
    #[serial]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn unix_default_is_dot_config_tokmesh_under_home() {
        let prev = save_env();
        unsafe {
            env::remove_var("TOKMESH_CONFIG_DIR");
            env::remove_var("XDG_CONFIG_HOME");
            env::set_var("HOME", "/tmp/tokmesh-core-paths-home");
        }
        assert_eq!(
            get_config_dir(),
            PathBuf::from("/tmp/tokmesh-core-paths-home/.config/tokmesh"),
        );
        restore_env(prev);
    }

    #[test]
    #[serial]
    #[cfg(target_os = "linux")]
    fn linux_honors_xdg_config_home_when_set() {
        let prev = save_env();
        unsafe {
            env::remove_var("TOKMESH_CONFIG_DIR");
            env::set_var("XDG_CONFIG_HOME", "/tmp/tokmesh-core-paths-xdg");
        }
        assert_eq!(
            get_config_dir(),
            PathBuf::from("/tmp/tokmesh-core-paths-xdg/tokmesh"),
        );
        restore_env(prev);
    }

    #[test]
    #[serial]
    fn cache_dir_is_cache_subdir_of_config_dir() {
        let prev = save_env();
        unsafe {
            env::set_var("TOKMESH_CONFIG_DIR", "/tmp/tokmesh-cache-test");
        }
        assert_eq!(
            get_cache_dir(),
            PathBuf::from("/tmp/tokmesh-cache-test/cache")
        );
        restore_env(prev);
    }

    #[test]
    #[serial]
    fn get_config_dir_treats_empty_override_as_unset() {
        // Empty TOKMESH_CONFIG_DIR previously slipped through and
        // produced PathBuf::from(""), which silently relocated cache
        // writes to ./cache and ./.tokmesh. The resolver must agree
        // with override detection: empty == unset.
        let prev = save_env();
        unsafe {
            env::set_var("TOKMESH_CONFIG_DIR", "");
        }
        let resolved = get_config_dir();
        assert_ne!(
            resolved,
            PathBuf::from(""),
            "empty override must not resolve to the empty path"
        );
        assert!(
            resolved.is_absolute() || resolved == Path::new(".tokmesh"),
            "empty override must fall through to platform default, got {resolved:?}"
        );
        restore_env(prev);
    }

    #[test]
    #[serial]
    fn is_config_dir_overridden_treats_empty_string_as_unset() {
        let prev = save_env();
        unsafe {
            env::set_var("TOKMESH_CONFIG_DIR", "");
        }
        assert!(!is_config_dir_overridden());
        restore_env(prev);
    }
}
