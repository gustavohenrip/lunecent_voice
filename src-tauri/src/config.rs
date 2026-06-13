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
    Groq,
}

impl Default for LlmBackend {
    fn default() -> Self {
        LlmBackend::Local
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionBackend {
    Local,
    Groq,
}

impl Default for TranscriptionBackend {
    fn default() -> Self {
        TranscriptionBackend::Local
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
    pub transcription_backend: TranscriptionBackend,
    pub groq_api_key: String,
    pub groq_model: String,
    pub groq_llm_model: String,
    pub groq_reuse_transcription_key: bool,
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
    pub llm_format_paragraphs: bool,
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
            transcription_backend: TranscriptionBackend::Local,
            groq_api_key: String::new(),
            groq_model: "whisper-large-v3-turbo".to_string(),
            groq_llm_model: "llama-3.1-8b-instant".to_string(),
            groq_reuse_transcription_key: true,
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
            llm_format_paragraphs: false,
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

pub enum LoadOutcome {
    Loaded(Box<Settings>),
    Missing,
    Corrupt,
}

enum ReadResult {
    Parsed(Box<Settings>),
    Missing,
    Bad(String),
}

fn read_and_parse(path: &Path) -> ReadResult {
    let mut last = String::new();
    for attempt in 0..4 {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                if content.trim().is_empty() {
                    last = content;
                } else {
                    match serde_json::from_str::<Settings>(&content) {
                        Ok(settings) => return ReadResult::Parsed(Box::new(settings)),
                        Err(err) => {
                            tracing::warn!("settings parse failed (attempt {attempt}): {err}");
                            last = content;
                        }
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return ReadResult::Missing,
            Err(err) => tracing::warn!("settings read failed (attempt {attempt}): {err}"),
        }
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
    }
    ReadResult::Bad(last)
}

impl Settings {
    pub fn load(path: &Path) -> LoadOutcome {
        match read_and_parse(path) {
            ReadResult::Parsed(settings) => LoadOutcome::Loaded(settings),
            ReadResult::Missing => match read_and_parse(&bak_path(path)) {
                ReadResult::Parsed(settings) => {
                    tracing::warn!("settings.json missing; recovered from settings.bak");
                    LoadOutcome::Loaded(settings)
                }
                _ => LoadOutcome::Missing,
            },
            ReadResult::Bad(content) => match read_and_parse(&bak_path(path)) {
                ReadResult::Parsed(settings) => {
                    tracing::warn!("settings.json unreadable; recovered from settings.bak");
                    LoadOutcome::Loaded(settings)
                }
                _ => {
                    backup_corrupt(path, &content);
                    LoadOutcome::Corrupt
                }
            },
        }
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        let json = serde_json::to_string_pretty(self)?;
        crate::atomic_io::write_durable(path, json.as_bytes())
            .map_err(|err| AppError::Config(err.to_string()))?;
        if let Err(err) = crate::atomic_io::write_durable(&bak_path(path), json.as_bytes()) {
            tracing::warn!("settings.bak write failed: {err}");
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

fn bak_path(path: &Path) -> std::path::PathBuf {
    path.with_extension("bak")
}

fn backup_corrupt(path: &Path, content: &str) {
    if content.is_empty() {
        return;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_extension(format!("corrupt-{stamp}.json"));
    if let Err(err) = std::fs::write(&backup, content) {
        tracing::warn!("failed to save corrupt settings backup: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    fn tmp_path() -> std::path::PathBuf {
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("lunecent_cfg_{}_{n}.json", std::process::id()))
    }

    fn cleanup(p: &Path) {
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(bak_path(p));
    }

    #[test]
    fn save_then_load_roundtrip() {
        let p = tmp_path();
        let mut s = Settings::default();
        s.groq_api_key = "gsk_test".to_string();
        s.llm_format_paragraphs = true;
        s.save(&p).unwrap();
        match Settings::load(&p) {
            LoadOutcome::Loaded(loaded) => {
                assert_eq!(loaded.groq_api_key, "gsk_test");
                assert!(loaded.llm_format_paragraphs);
            }
            _ => panic!("expected Loaded"),
        }
        cleanup(&p);
    }

    #[test]
    fn missing_file_is_missing() {
        let p = tmp_path();
        assert!(matches!(Settings::load(&p), LoadOutcome::Missing));
    }

    #[test]
    fn corrupt_without_backup_preserves_original() {
        let p = tmp_path();
        std::fs::write(&p, b"{ not valid json ").unwrap();
        assert!(matches!(Settings::load(&p), LoadOutcome::Corrupt));
        let still = std::fs::read_to_string(&p).unwrap();
        assert!(still.contains("not valid"));
        cleanup(&p);
    }

    #[test]
    fn corrupt_primary_recovers_from_backup() {
        let p = tmp_path();
        let mut s = Settings::default();
        s.groq_api_key = "from_bak".to_string();
        s.save(&p).unwrap();
        std::fs::write(&p, b"garbage").unwrap();
        match Settings::load(&p) {
            LoadOutcome::Loaded(loaded) => assert_eq!(loaded.groq_api_key, "from_bak"),
            _ => panic!("expected recovery from settings.bak"),
        }
        cleanup(&p);
    }

    #[test]
    fn empty_file_is_corrupt_not_missing() {
        let p = tmp_path();
        std::fs::write(&p, b"").unwrap();
        assert!(matches!(Settings::load(&p), LoadOutcome::Corrupt));
        cleanup(&p);
    }
}
