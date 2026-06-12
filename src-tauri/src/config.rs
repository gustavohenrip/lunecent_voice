use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordMode {
    PushToTalk,
    Toggle,
}

impl Default for RecordMode {
    fn default() -> Self {
        RecordMode::PushToTalk
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmBackend {
    Local,
    OpenAiCompatible,
    Anthropic,
    Ollama,
}

impl Default for LlmBackend {
    fn default() -> Self {
        LlmBackend::Local
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey_ptt: String,
    pub hotkey_toggle: String,
    pub record_mode: RecordMode,
    pub language: String,
    pub whisper_model: String,
    pub audio_device: Option<String>,
    pub vad_enabled: bool,
    pub vad_threshold: f32,
    pub min_silence_ms: u32,
    pub speech_pad_ms: u32,
    pub filler_removal: bool,
    pub filler_words: Vec<String>,
    pub dictionary: BTreeMap<String, String>,
    pub vocabulary: Vec<String>,
    pub llm_enabled: bool,
    pub translation_enabled: bool,
    pub translation_target: String,
    pub llm_backend: LlmBackend,
    pub llm_local_model: String,
    pub llm_endpoint: String,
    pub llm_api_key: String,
    pub llm_model_name: String,
    pub llm_timeout_ms: u64,
    pub llm_temperature: f32,
    pub llm_gpu_layers: i32,
    pub autostart: bool,
    pub restore_clipboard: bool,
    pub paste_delay_ms: u64,
    pub prefer_gpu: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hotkey_ptt: "Ctrl+Shift+Space".to_string(),
            hotkey_toggle: "Ctrl+Shift+T".to_string(),
            record_mode: RecordMode::PushToTalk,
            language: "auto".to_string(),
            whisper_model: "large-v3-turbo".to_string(),
            audio_device: None,
            vad_enabled: true,
            vad_threshold: 0.5,
            min_silence_ms: 200,
            speech_pad_ms: 120,
            filler_removal: true,
            filler_words: default_fillers(),
            dictionary: BTreeMap::new(),
            vocabulary: Vec::new(),
            llm_enabled: true,
            translation_enabled: false,
            translation_target: "English".to_string(),
            llm_backend: LlmBackend::Local,
            llm_local_model: "google_gemma-3-4b-it-Q4_K_M.gguf".to_string(),
            llm_endpoint: "http://127.0.0.1:8123/v1".to_string(),
            llm_api_key: String::new(),
            llm_model_name: "local".to_string(),
            llm_timeout_ms: 2000,
            llm_temperature: 0.1,
            llm_gpu_layers: 99,
            autostart: false,
            restore_clipboard: true,
            paste_delay_ms: 120,
            prefer_gpu: true,
        }
    }
}

fn default_fillers() -> Vec<String> {
    [
        "aham", "ahn", "hum", "humm", "uhm", "uh", "uhh", "hmm", "ééé", "éé", "eh", "er", "mmm",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl Settings {
    pub fn load(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Settings>(&content) {
                Ok(settings) => settings,
                Err(err) => {
                    tracing::warn!("settings parse failed ({err}); using defaults");
                    let _ = backup_corrupt(path);
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        static SAVE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let seq = SAVE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = path.with_extension(format!("json.tmp-{}-{seq}", std::process::id()));
        std::fs::write(&tmp, json.as_bytes())?;
        if let Err(err) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::Config(err.to_string()));
        }
        Ok(())
    }

    pub fn language_code(&self) -> Option<String> {
        match self.language.as_str() {
            "auto" | "" => None,
            other => Some(other.to_string()),
        }
    }

    pub fn whisper_translate(&self) -> bool {
        self.translation_enabled && self.translation_target.trim().eq_ignore_ascii_case("english")
    }
}

fn backup_corrupt(path: &Path) -> AppResult<()> {
    if path.exists() {
        let backup = path.with_extension("json.corrupt");
        std::fs::copy(path, backup)?;
    }
    Ok(())
}
