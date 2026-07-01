use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub xai_api_key: String,
    #[serde(default)]
    pub input_device_name: Option<String>,
    #[serde(default = "default_silence_duration_secs")]
    pub silence_duration_secs: f32,
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold: f32,
    #[serde(default = "default_clipboard_enabled")]
    pub clipboard_enabled: bool,
    #[serde(default = "default_auto_input_enabled")]
    pub auto_input_enabled: bool,
    #[serde(default = "default_auto_input_send_enter")]
    pub auto_input_send_enter: bool,
    #[serde(default = "default_vrchat_enabled")]
    pub vrchat_enabled: bool,
    #[serde(default = "default_eliza_enabled")]
    pub eliza_enabled: bool,
    #[serde(default = "default_eliza_response_to_vrchat_enabled")]
    pub eliza_response_to_vrchat_enabled: bool,
    #[serde(default = "default_eliza_url")]
    pub eliza_url: String,
    #[serde(default = "default_auto_translate_enabled")]
    pub auto_translate_enabled: bool,
    #[serde(default = "default_translate_lang_preset")]
    pub translate_lang_preset: String,
    #[serde(default = "default_translate_lang_custom")]
    pub translate_lang_custom: String,
}

fn default_silence_duration_secs() -> f32 {
    2.0
}

fn default_silence_threshold() -> f32 {
    0.01
}

fn default_clipboard_enabled() -> bool {
    true
}

fn default_auto_input_enabled() -> bool {
    false
}

fn default_auto_input_send_enter() -> bool {
    false
}

fn default_vrchat_enabled() -> bool {
    false
}

fn default_eliza_enabled() -> bool {
    false
}

fn default_eliza_response_to_vrchat_enabled() -> bool {
    false
}

fn default_eliza_url() -> String {
    "http://localhost:9096".to_string()
}

fn default_auto_translate_enabled() -> bool {
    false
}

fn default_translate_lang_preset() -> String {
    "EN".to_string()
}

fn default_translate_lang_custom() -> String {
    String::new()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            xai_api_key: String::new(),
            input_device_name: None,
            silence_duration_secs: default_silence_duration_secs(),
            silence_threshold: default_silence_threshold(),
            clipboard_enabled: default_clipboard_enabled(),
            auto_input_enabled: default_auto_input_enabled(),
            auto_input_send_enter: default_auto_input_send_enter(),
            vrchat_enabled: default_vrchat_enabled(),
            eliza_enabled: default_eliza_enabled(),
            eliza_response_to_vrchat_enabled: default_eliza_response_to_vrchat_enabled(),
            eliza_url: default_eliza_url(),
            auto_translate_enabled: default_auto_translate_enabled(),
            translate_lang_preset: default_translate_lang_preset(),
            translate_lang_custom: default_translate_lang_custom(),
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf, String> {
        let config_dir = dirs::config_dir().ok_or("Failed to get config directory")?;
        let app_config_dir = config_dir.join("vrc-companion");

        if !app_config_dir.exists() {
            fs::create_dir_all(&app_config_dir)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        Ok(app_config_dir.join("config.json"))
    }

    pub fn load() -> Self {
        match Self::config_path() {
            Ok(path) if path.exists() => match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                    eprintln!("Failed to parse config: {}", e);
                    Self::default()
                }),
                Err(e) => {
                    eprintln!("Failed to read config file: {}", e);
                    Self::default()
                }
            },
            Ok(_) => Self::default(),
            Err(e) => {
                eprintln!("Failed to get config path: {}", e);
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(&path, json).map_err(|e| format!("Failed to write config file: {}", e))?;
        Ok(())
    }

    /// auto_input と vrchat は排他 (5と3は排反)。一方を有効にしたらもう一方を無効化する。
    pub fn enable_auto_input_exclusive(&mut self) {
        self.auto_input_enabled = true;
        self.vrchat_enabled = false;
    }

    pub fn enable_vrchat_exclusive(&mut self) {
        self.vrchat_enabled = true;
        self.auto_input_enabled = false;
    }

    /// eliza (通常会話) と auto_translate (自動翻訳) は排他。一方を有効にしたらもう一方を無効化する。
    pub fn enable_eliza_exclusive(&mut self) {
        self.eliza_enabled = true;
        self.auto_translate_enabled = false;
    }

    pub fn enable_auto_translate_exclusive(&mut self) {
        self.auto_translate_enabled = true;
        self.eliza_enabled = false;
    }

    /// 翻訳先言語の表示名を preset/custom から解決する
    pub fn translate_target_lang(&self) -> String {
        match self.translate_lang_preset.as_str() {
            "EN" => "英語".to_string(),
            "CN" => "中国語".to_string(),
            _ => self.translate_lang_custom.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.silence_duration_secs, 2.0);
        assert_eq!(config.silence_threshold, 0.01);
        assert!(config.clipboard_enabled);
        assert!(!config.auto_input_enabled);
        assert!(!config.vrchat_enabled);
        assert!(!config.eliza_enabled);
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut config = Config::default();
        config.xai_api_key = "secret".to_string();
        config.vrchat_enabled = true;
        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.xai_api_key, "secret");
        assert!(restored.vrchat_enabled);
    }

    #[test]
    fn test_load_missing_fields_uses_defaults() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config.eliza_url, "http://localhost:9096");
        assert!(config.clipboard_enabled);
        assert!(!config.eliza_response_to_vrchat_enabled);
    }

    #[test]
    fn test_eliza_response_to_vrchat_serde_roundtrip() {
        let mut config = Config::default();
        config.eliza_response_to_vrchat_enabled = true;
        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();
        assert!(restored.eliza_response_to_vrchat_enabled);
    }

    #[test]
    fn test_auto_input_and_vrchat_are_mutually_exclusive() {
        let mut config = Config::default();
        config.enable_vrchat_exclusive();
        assert!(config.vrchat_enabled);
        assert!(!config.auto_input_enabled);

        config.enable_auto_input_exclusive();
        assert!(config.auto_input_enabled);
        assert!(!config.vrchat_enabled);
    }

    #[test]
    fn test_eliza_and_auto_translate_are_mutually_exclusive() {
        let mut config = Config::default();
        config.enable_auto_translate_exclusive();
        assert!(config.auto_translate_enabled);
        assert!(!config.eliza_enabled);

        config.enable_eliza_exclusive();
        assert!(config.eliza_enabled);
        assert!(!config.auto_translate_enabled);
    }

    #[test]
    fn test_translate_target_lang_presets() {
        let mut config = Config::default();
        assert_eq!(config.translate_target_lang(), "英語");

        config.translate_lang_preset = "CN".to_string();
        assert_eq!(config.translate_target_lang(), "中国語");

        config.translate_lang_preset = "CUSTOM".to_string();
        config.translate_lang_custom = "フランス語".to_string();
        assert_eq!(config.translate_target_lang(), "フランス語");
    }

    #[test]
    fn test_auto_translate_serde_roundtrip() {
        let mut config = Config::default();
        config.auto_translate_enabled = true;
        config.translate_lang_preset = "CN".to_string();
        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();
        assert!(restored.auto_translate_enabled);
        assert_eq!(restored.translate_lang_preset, "CN");
    }
}
