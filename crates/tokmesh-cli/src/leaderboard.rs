//! Public leaderboard targets (tokscale.ai and tokens.ci).

use std::fmt;
use std::path::PathBuf;

/// Which public leaderboard a login/submit command targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Leaderboard {
    Tokscale,
    TokensCi,
}

impl Leaderboard {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tokscale => "tokscale",
            Self::TokensCi => "tokensci",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Tokscale => "tokscale.ai",
            Self::TokensCi => "tokens.ci",
        }
    }

    pub fn dir_name(self) -> &'static str {
        self.as_str()
    }

    pub fn autosubmit_job_id(self) -> &'static str {
        match self {
            Self::Tokscale => "ai.tokmesh.autosubmit.tokscale",
            Self::TokensCi => "ai.tokmesh.autosubmit.tokensci",
        }
    }

    pub fn autosubmit_systemd_stem(self) -> &'static str {
        match self {
            Self::Tokscale => "tokmesh-tokscale-autosubmit",
            Self::TokensCi => "tokmesh-tokensci-autosubmit",
        }
    }

    pub fn autosubmit_cron_markers(self) -> (&'static str, &'static str) {
        match self {
            Self::Tokscale => (
                "# BEGIN TOKMESH TOKSCALE AUTOSUBMIT",
                "# END TOKMESH TOKSCALE AUTOSUBMIT",
            ),
            Self::TokensCi => (
                "# BEGIN TOKMESH TOKENSCI AUTOSUBMIT",
                "# END TOKMESH TOKENSCI AUTOSUBMIT",
            ),
        }
    }

    pub fn default_api_base_url(self) -> &'static str {
        match self {
            Self::Tokscale => "https://tokscale.ai",
            Self::TokensCi => "https://tokens.ci",
        }
    }

    pub fn api_url_env(self) -> &'static str {
        match self {
            Self::Tokscale => "TOKMESH_TOKSCALE_API_URL",
            Self::TokensCi => "TOKMESH_TOKENSCI_API_URL",
        }
    }

    pub fn api_token_env(self) -> &'static str {
        match self {
            Self::Tokscale => "TOKMESH_TOKSCALE_API_TOKEN",
            Self::TokensCi => "TOKMESH_TOKENSCI_API_TOKEN",
        }
    }

    pub fn credentials_path(self) -> PathBuf {
        crate::paths::get_config_dir()
            .join(self.dir_name())
            .join("credentials.json")
    }

    pub fn api_base_url(self) -> String {
        std::env::var(self.api_url_env())
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| self.default_api_base_url().to_string())
    }
}

impl fmt::Display for Leaderboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn defaults_point_at_public_hosts() {
        assert_eq!(
            Leaderboard::Tokscale.default_api_base_url(),
            "https://tokscale.ai"
        );
        assert_eq!(
            Leaderboard::TokensCi.default_api_base_url(),
            "https://tokens.ci"
        );
    }

    #[test]
    #[serial]
    fn credentials_live_under_config_subdir() {
        let tmp = TempDir::new().unwrap();
        let prev = env::var_os("TOKMESH_CONFIG_DIR");
        unsafe {
            env::set_var("TOKMESH_CONFIG_DIR", tmp.path());
        }
        assert_eq!(
            Leaderboard::Tokscale.credentials_path(),
            tmp.path().join("tokscale/credentials.json")
        );
        assert_eq!(
            Leaderboard::TokensCi.credentials_path(),
            tmp.path().join("tokensci/credentials.json")
        );
        unsafe {
            match prev {
                Some(v) => env::set_var("TOKMESH_CONFIG_DIR", v),
                None => env::remove_var("TOKMESH_CONFIG_DIR"),
            }
        }
    }

    #[test]
    #[serial]
    fn api_url_uses_board_specific_env() {
        for name in ["TOKMESH_TOKSCALE_API_URL", "TOKMESH_TOKENSCI_API_URL"] {
            unsafe { env::remove_var(name) };
        }

        unsafe {
            env::set_var("TOKMESH_TOKSCALE_API_URL", "http://127.0.0.1:4201");
            env::set_var("TOKMESH_TOKENSCI_API_URL", "http://127.0.0.1:4202");
        }
        assert_eq!(
            Leaderboard::Tokscale.api_base_url(),
            "http://127.0.0.1:4201"
        );
        assert_eq!(
            Leaderboard::TokensCi.api_base_url(),
            "http://127.0.0.1:4202"
        );

        for name in ["TOKMESH_TOKSCALE_API_URL", "TOKMESH_TOKENSCI_API_URL"] {
            unsafe { env::remove_var(name) };
        }
    }
}
