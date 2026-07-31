use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::IsTerminal;
use std::io::Write;
use std::path::PathBuf;

use crate::leaderboard::Leaderboard;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub token: String,
    pub username: String,
    #[serde(rename = "avatarUrl", skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiTokenSource {
    Environment,
    StoredCredentials,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiTokenAuth {
    pub token: String,
    pub username: Option<String>,
    pub source: ApiTokenSource,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    #[serde(rename = "deviceCode")]
    device_code: String,
    #[serde(rename = "userCode")]
    user_code: String,
    #[serde(rename = "verificationUrl")]
    verification_url: String,
    #[serde(rename = "expiresIn")]
    expires_in: u64,
    interval: u64,
}

/// Bounds on the server-supplied device-flow poll interval.
const MIN_POLL_INTERVAL_SECS: u64 = 1;
const MAX_POLL_INTERVAL_SECS: u64 = 60;
const MIN_CODE_LIFETIME_SECS: u64 = 60;
const MAX_CODE_LIFETIME_SECS: u64 = 30 * 60;

#[derive(Debug, Deserialize)]
struct PollResponse {
    // Optional: some error paths omit `status` entirely.
    #[serde(default)]
    status: Option<String>,
    token: Option<String>,
    user: Option<UserInfo>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    username: String,
    #[serde(rename = "avatarUrl")]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenValidationResponse {
    user: UserInfo,
}

fn get_credentials_path(board: Leaderboard) -> PathBuf {
    board.credentials_path()
}

fn ensure_credentials_parent(board: Leaderboard) -> Result<()> {
    let path = get_credentials_path(board);
    let parent = path.parent().context("credentials path has no parent")?;
    if !parent.exists() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

pub fn save_credentials(board: Leaderboard, credentials: &Credentials) -> Result<()> {
    ensure_credentials_parent(board)?;
    let path = get_credentials_path(board);
    let json = serde_json::to_string_pretty(credentials)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        // `.mode()` only applies on create; repair perms on every write.
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(json.as_bytes())?;
    }

    #[cfg(not(unix))]
    {
        fs::write(&path, json)?;
    }

    Ok(())
}

pub fn load_credentials(board: Leaderboard) -> Option<Credentials> {
    let path = get_credentials_path(board);
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn load_api_token_from_env(board: Leaderboard) -> Option<String> {
    std::env::var(board.api_token_env())
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

pub fn resolve_api_token(board: Leaderboard) -> Option<ApiTokenAuth> {
    if let Some(token) = load_api_token_from_env(board) {
        return Some(ApiTokenAuth {
            token,
            username: None,
            source: ApiTokenSource::Environment,
        });
    }

    load_credentials(board).map(|credentials| ApiTokenAuth {
        token: credentials.token,
        username: Some(credentials.username),
        source: ApiTokenSource::StoredCredentials,
    })
}

pub fn clear_credentials(board: Leaderboard) -> Result<bool> {
    let path = get_credentials_path(board);
    if path.exists() {
        fs::remove_file(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn get_api_base_url(board: Leaderboard) -> String {
    board.api_base_url()
}

fn get_device_name() -> String {
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());
    format!("CLI on {}", hostname)
}

#[cfg(target_os = "linux")]
fn has_non_empty_env_var(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

#[cfg(target_os = "linux")]
fn should_auto_open_browser() -> bool {
    has_non_empty_env_var("DISPLAY") || has_non_empty_env_var("WAYLAND_DISPLAY")
}

#[cfg(not(target_os = "linux"))]
fn should_auto_open_browser() -> bool {
    true
}

fn open_browser(url: &str) -> bool {
    if !should_auto_open_browser() {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        return std::process::Command::new("open").arg(url).spawn().is_ok();
    }

    #[cfg(target_os = "windows")]
    {
        return std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .is_ok();
    }

    #[cfg(target_os = "linux")]
    {
        return std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .is_ok();
    }

    #[allow(unreachable_code)]
    false
}

fn clamp_u64(value: u64, min: u64, max: u64) -> u64 {
    value.clamp(min, max)
}

pub async fn login(board: Leaderboard) -> Result<()> {
    use colored::Colorize;

    if let Some(creds) = load_credentials(board) {
        println!(
            "\n  {}",
            format!(
                "Already logged in to {} as {}",
                board,
                creds.username.bold()
            )
            .yellow()
        );
        println!(
            "{}",
            format!(
                "  Run 'tokmesh {} logout' to sign out first.\n",
                board.as_str()
            )
            .bright_black()
        );
        return Ok(());
    }

    let base_url = get_api_base_url(board);

    println!("\n  {}\n", format!("Tokmesh → {} — Login", board).cyan());
    println!("{}", "  Requesting authorization code...".bright_black());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let device_code_response = client
        .post(format!("{}/api/auth/device", base_url))
        .json(&serde_json::json!({
            "deviceName": get_device_name()
        }))
        .send()
        .await?;

    if !device_code_response.status().is_success() {
        anyhow::bail!("Server returned {}", device_code_response.status());
    }

    let device_data: DeviceCodeResponse = device_code_response.json().await?;
    let poll_interval = std::time::Duration::from_secs(clamp_u64(
        device_data.interval,
        MIN_POLL_INTERVAL_SECS,
        MAX_POLL_INTERVAL_SECS,
    ));
    let lifetime_secs = clamp_u64(
        device_data.expires_in,
        MIN_CODE_LIFETIME_SECS,
        MAX_CODE_LIFETIME_SECS,
    );
    let max_attempts = (lifetime_secs / poll_interval.as_secs().max(1)).max(1);

    println!();
    println!("{}", "  Open this URL in your browser:".white());
    let url_display = if std::io::stdout().is_terminal() {
        format!(
            "\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\",
            device_data.verification_url, device_data.verification_url
        )
    } else {
        device_data.verification_url.clone()
    };
    println!("{}", format!("  {}\n", url_display).cyan());
    println!("{}", "  Enter this code:".white());
    println!(
        "{}\n",
        format!("  {}", device_data.user_code).green().bold()
    );

    if !open_browser(&device_data.verification_url) {
        println!(
            "{}",
            "  Browser auto-open unavailable in this environment. Continue with the URL above.\n"
                .bright_black()
        );
    }

    println!("{}", "  Waiting for authorization...".bright_black());

    for attempt in 0..max_attempts {
        tokio::time::sleep(poll_interval).await;

        let poll_response = client
            .post(format!("{}/api/auth/device/poll", base_url))
            .json(&serde_json::json!({
                "deviceCode": device_data.device_code
            }))
            .send()
            .await;

        match poll_response {
            Ok(response) => {
                if let Ok(data) = response.json::<PollResponse>().await {
                    if let Some(err) = data.error.as_deref() {
                        if data.status.as_deref() != Some("pending")
                            && data.status.as_deref() != Some("complete")
                        {
                            // Surface server error when present and not mid-flow.
                            if data.status.as_deref() == Some("expired") || data.token.is_none() {
                                if data.status.as_deref() == Some("expired") {
                                    anyhow::bail!("Authorization code expired. Please try again.");
                                }
                            }
                            let _ = err;
                        }
                    }

                    let status = data.status.as_deref().unwrap_or("");
                    if status == "complete" {
                        if let (Some(token), Some(user)) = (data.token, data.user) {
                            let credentials = Credentials {
                                token,
                                username: user.username.clone(),
                                avatar_url: user.avatar_url,
                                created_at: chrono::Utc::now().to_rfc3339(),
                            };

                            save_credentials(board, &credentials)?;

                            println!(
                                "\n  {}",
                                format!(
                                    "Success! Logged in to {} as {}",
                                    board,
                                    user.username.bold()
                                )
                                .green()
                            );
                            println!(
                                "{}",
                                format!(
                                    "  You can now use 'tokmesh {} submit' to share your usage.\n",
                                    board.as_str()
                                )
                                .bright_black()
                            );
                            return Ok(());
                        }
                    }

                    if status == "expired" {
                        anyhow::bail!("Authorization code expired. Please try again.");
                    }

                    print!("{}", ".".bright_black());
                    use std::io::Write;
                    std::io::stdout().flush()?;
                }
            }
            Err(_) => {
                print!("{}", "!".red());
                use std::io::Write;
                std::io::stdout().flush()?;
            }
        }

        if attempt >= max_attempts - 1 {
            anyhow::bail!("Timeout: Authorization took too long. Please try again.");
        }
    }

    Ok(())
}

pub async fn login_with_token(board: Leaderboard, token: &str) -> Result<()> {
    use colored::Colorize;

    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("API token cannot be empty.");
    }
    if !token.starts_with("tt_") {
        anyhow::bail!("Leaderboard API tokens must start with `tt_`.");
    }

    let base_url = get_api_base_url(board);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client
        .get(format!("{}/api/auth/token", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        let error = body
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("API token validation failed");
        anyhow::bail!("{} ({})", error, status);
    }

    let data: TokenValidationResponse = response.json().await?;
    let credentials = Credentials {
        token: token.to_string(),
        username: data.user.username.clone(),
        avatar_url: data.user.avatar_url,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    save_credentials(board, &credentials)?;

    println!(
        "\n  {}",
        format!(
            "Success! Logged in to {} as {}",
            board,
            credentials.username.bold()
        )
        .green()
    );
    println!(
        "{}",
        format!(
            "  You can now use 'tokmesh {} submit' to share your usage.\n",
            board.as_str()
        )
        .bright_black()
    );

    Ok(())
}

pub fn logout(board: Leaderboard) -> Result<()> {
    use colored::Colorize;

    let credentials = load_credentials(board);

    let Some(creds) = credentials else {
        println!("\n  {}\n", format!("Not logged in to {}.", board).yellow());
        return Ok(());
    };

    let username = creds.username;
    let cleared = clear_credentials(board)?;

    if cleared {
        println!(
            "\n  {}\n",
            format!("Logged out of {} ({})", board, username.bold()).green()
        );
    } else {
        anyhow::bail!("Failed to clear credentials.");
    }

    Ok(())
}

pub fn whoami(board: Leaderboard) -> Result<()> {
    use colored::Colorize;

    let Some(creds) = load_credentials(board) else {
        println!("\n  {}", format!("Not logged in to {}.", board).yellow());
        println!(
            "{}",
            format!(
                "  Run 'tokmesh {} login' to authenticate.\n",
                board.as_str()
            )
            .bright_black()
        );
        return Ok(());
    };

    println!("\n  {}\n", format!("Tokmesh → {} — Account", board).cyan());
    println!(
        "{}",
        format!("  Username:  {}", creds.username.bold()).white()
    );

    if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&creds.created_at) {
        println!(
            "{}",
            format!("  Logged in: {}", created.format("%Y-%m-%d")).bright_black()
        );
    }

    println!();

    Ok(())
}

/// Build the JSON payload encoded into the login QR code.
pub(crate) fn qr_login_payload(token: &str, username: &str) -> Result<String> {
    serde_json::to_string(&serde_json::json!({
        "token": token,
        "username": username,
    }))
    .context("Failed to encode QR payload")
}

pub fn show_qr(board: Leaderboard, yes: bool) -> Result<()> {
    use colored::Colorize;
    use qrcode::render::unicode;
    use qrcode::QrCode;

    let Some(creds) = load_credentials(board) else {
        println!("\n  {}", format!("Not logged in to {}.", board).yellow());
        println!(
            "{}",
            format!(
                "  Run 'tokmesh {} login' to authenticate.\n",
                board.as_str()
            )
            .bright_black()
        );
        return Ok(());
    };

    println!();
    println!(
        "  {}",
        "⚠  This will render your API token as a QR code on screen.".yellow()
    );
    println!(
        "  {}",
        "Anyone who can see your terminal can scan it and access this leaderboard account."
            .bright_black()
    );
    println!();

    if !yes {
        if !std::io::stdin().is_terminal() {
            anyhow::bail!("Refusing to render token QR: stdin is not a TTY. Pass --yes to bypass.");
        }
        print!("  Continue? [y/N] ");
        std::io::stdout().flush().ok();
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .context("Failed to read confirmation")?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("\n  {}\n", "Aborted.".bright_black());
            return Ok(());
        }
    }

    let payload = qr_login_payload(&creds.token, &creds.username)?;
    let code = QrCode::new(payload.as_bytes()).context("Failed to generate QR code")?;

    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build();

    println!(
        "\n  {}\n",
        format!("Tokmesh → {} — API Token QR", board).cyan()
    );
    println!("  {}\n", "Scan to get your API token:".bright_black());

    for line in image.lines() {
        println!("  {}", line);
    }

    println!("\n  {}: {}\n", "User".bright_black(), creds.username.bold());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use tempfile::TempDir;

    fn with_config_dir<T>(f: impl FnOnce(&TempDir) -> T) -> T {
        let tmp = TempDir::new().unwrap();
        let prev = env::var_os("TOKMESH_CONFIG_DIR");
        unsafe {
            env::set_var("TOKMESH_CONFIG_DIR", tmp.path());
        }
        let out = f(&tmp);
        unsafe {
            match prev {
                Some(v) => env::set_var("TOKMESH_CONFIG_DIR", v),
                None => env::remove_var("TOKMESH_CONFIG_DIR"),
            }
        }
        out
    }

    #[test]
    #[serial]
    fn credentials_are_isolated_per_leaderboard() {
        with_config_dir(|tmp| {
            let a = Credentials {
                token: "tt_a".into(),
                username: "alice".into(),
                avatar_url: None,
                created_at: "2024-01-01T00:00:00Z".into(),
            };
            let b = Credentials {
                token: "tt_b".into(),
                username: "bob".into(),
                avatar_url: None,
                created_at: "2024-01-01T00:00:00Z".into(),
            };
            save_credentials(Leaderboard::Tokscale, &a).unwrap();
            save_credentials(Leaderboard::TokensCi, &b).unwrap();
            assert_eq!(
                load_credentials(Leaderboard::Tokscale).unwrap().username,
                "alice"
            );
            assert_eq!(
                load_credentials(Leaderboard::TokensCi).unwrap().username,
                "bob"
            );
            assert!(tmp.path().join("tokscale/credentials.json").exists());
            assert!(tmp.path().join("tokensci/credentials.json").exists());
        });
    }

    #[test]
    #[serial]
    fn resolve_prefers_board_specific_env() {
        with_config_dir(|_| {
            unsafe {
                env::remove_var("TOKMESH_TOKSCALE_API_TOKEN");
                env::remove_var("TOKSCALE_API_TOKEN");
                env::set_var("TOKMESH_TOKSCALE_API_TOKEN", "tt_env");
            }
            let auth = resolve_api_token(Leaderboard::Tokscale).unwrap();
            assert_eq!(auth.token, "tt_env");
            assert_eq!(auth.source, ApiTokenSource::Environment);
            unsafe {
                env::remove_var("TOKMESH_TOKSCALE_API_TOKEN");
            }
        });
    }

    #[test]
    #[serial]
    fn api_base_url_defaults_and_overrides() {
        with_config_dir(|_| {
            unsafe {
                env::remove_var("TOKMESH_TOKSCALE_API_URL");
                env::remove_var("TOKSCALE_API_URL");
                env::remove_var("TOKMESH_TOKENSCI_API_URL");
                env::remove_var("TOKENS_API_URL");
            }
            assert_eq!(
                get_api_base_url(Leaderboard::Tokscale),
                "https://tokscale.ai"
            );
            assert_eq!(get_api_base_url(Leaderboard::TokensCi), "https://tokens.ci");
            unsafe {
                env::set_var("TOKENS_API_URL", "http://127.0.0.1:8");
                env::set_var("TOKMESH_TOKENSCI_API_URL", "http://127.0.0.1:9");
            }
            assert_eq!(
                get_api_base_url(Leaderboard::TokensCi),
                "http://127.0.0.1:9"
            );
            unsafe {
                env::remove_var("TOKMESH_TOKENSCI_API_URL");
                env::remove_var("TOKENS_API_URL");
            }
        });
    }

    #[test]
    #[serial]
    fn qr_payload_is_json() {
        with_config_dir(|_| {
            let payload = qr_login_payload(r#"tt_"x"#, "user").unwrap();
            let v: serde_json::Value = serde_json::from_str(&payload).unwrap();
            assert_eq!(v["username"], "user");
        });
    }
}
