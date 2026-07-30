use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use ratatui::style::Color;
use serde::Deserialize;

static CONFIG: OnceLock<TokmeshConfig> = OnceLock::new();

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TokmeshConfig {
    #[serde(default)]
    pub colors: ColorsConfig,
    #[serde(default)]
    pub display_names: DisplayNamesConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ColorsConfig {
    #[serde(default)]
    pub providers: HashMap<String, String>,
    #[serde(default, alias = "sources")]
    pub clients: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DisplayNamesConfig {
    #[serde(default)]
    pub providers: HashMap<String, String>,
    #[serde(default, alias = "sources")]
    pub clients: HashMap<String, String>,
}

impl TokmeshConfig {
    fn config_path() -> PathBuf {
        crate::paths::get_config_dir().join("config.toml")
    }

    pub fn load() -> &'static TokmeshConfig {
        CONFIG.get_or_init(|| {
            fs::read_to_string(Self::config_path())
                .ok()
                .and_then(|content| toml::from_str(&content).ok())
                .unwrap_or_default()
        })
    }

    pub fn get_provider_color(&self, provider: &str) -> Option<Color> {
        self.colors
            .providers
            .get(&provider.to_lowercase())
            .and_then(|hex| parse_hex_color(hex))
    }

    pub fn get_client_color(&self, client: &str) -> Option<Color> {
        self.colors
            .clients
            .get(&client.to_lowercase())
            .and_then(|hex| parse_hex_color(hex))
    }

    pub fn get_provider_display_name(&self, provider: &str) -> Option<&str> {
        self.display_names
            .providers
            .get(&provider.to_lowercase())
            .map(|s| s.as_str())
    }

    pub fn get_client_display_name(&self, client: &str) -> Option<&str> {
        self.display_names
            .clients
            .get(&client.to_lowercase())
            .map(|s| s.as_str())
    }
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
