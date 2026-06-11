use crate::config::{LlmBackend, Settings};
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use std::time::Duration;

pub const SYSTEM_PROMPT: &str = "You receive a raw voice transcription that may contain mis-spoken words, wrong word order, repetitions, filler and self-corrections. Rewrite it as the text the speaker INTENDED. Preserve meaning, technical terms, names and casing exactly. Never add information. Return ONLY the corrected text, in the same language as the input.";

pub struct LlmClient {
    http: reqwest::Client,
}

impl LlmClient {
    pub fn new() -> LlmClient {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        LlmClient { http }
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub async fn cleanup(&self, settings: &Settings, raw: &str) -> String {
        if !settings.llm_enabled || raw.trim().is_empty() {
            return raw.to_string();
        }

        let timeout = Duration::from_millis(settings.llm_timeout_ms.clamp(200, 20000));
        match tokio::time::timeout(timeout, self.run(settings, raw)).await {
            Ok(Ok(cleaned)) => {
                let trimmed = cleaned.trim();
                if trimmed.is_empty() {
                    raw.to_string()
                } else {
                    sanitize(trimmed)
                }
            }
            Ok(Err(err)) => {
                tracing::warn!("llm cleanup failed ({err}); using raw text");
                raw.to_string()
            }
            Err(_) => {
                tracing::warn!("llm cleanup timed out; using raw text");
                raw.to_string()
            }
        }
    }

    async fn run(&self, settings: &Settings, raw: &str) -> AppResult<String> {
        match settings.llm_backend {
            LlmBackend::Anthropic => self.anthropic(settings, raw).await,
            _ => self.openai_compatible(settings, raw).await,
        }
    }

    async fn openai_compatible(&self, settings: &Settings, raw: &str) -> AppResult<String> {
        let base = settings.llm_endpoint.trim_end_matches('/');
        let url = format!("{base}/chat/completions");

        let body = json!({
            "model": settings.llm_model_name,
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": raw }
            ],
            "temperature": settings.llm_temperature,
            "max_tokens": 1024,
            "stream": false
        });

        let mut request = self.http.post(&url).json(&body);
        if !settings.llm_api_key.is_empty() {
            request = request.bearer_auth(&settings.llm_api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;
        let value = read_json(response).await?;
        extract_openai(&value).ok_or_else(|| AppError::Llm("unexpected response shape".to_string()))
    }

    async fn anthropic(&self, settings: &Settings, raw: &str) -> AppResult<String> {
        let base = settings.llm_endpoint.trim_end_matches('/');
        let url = format!("{base}/messages");

        let body = json!({
            "model": settings.llm_model_name,
            "max_tokens": 1024,
            "temperature": settings.llm_temperature,
            "system": SYSTEM_PROMPT,
            "messages": [ { "role": "user", "content": raw } ]
        });

        let response = self
            .http
            .post(&url)
            .header("x-api-key", &settings.llm_api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Llm(e.to_string()))?;
        let value = read_json(response).await?;
        extract_anthropic(&value)
            .ok_or_else(|| AppError::Llm("unexpected response shape".to_string()))
    }

    pub async fn test(&self, settings: &Settings) -> AppResult<String> {
        let probe = "test";
        self.run(settings, probe).await
    }
}

impl Default for LlmClient {
    fn default() -> Self {
        LlmClient::new()
    }
}

async fn read_json(response: reqwest::Response) -> AppResult<Value> {
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| AppError::Llm(e.to_string()))?;
    if !status.is_success() {
        let snippet: String = text.chars().take(300).collect();
        return Err(AppError::Llm(format!("{status}: {snippet}")));
    }
    serde_json::from_str(&text).map_err(|e| AppError::Llm(e.to_string()))
}

fn extract_openai(value: &Value) -> Option<String> {
    content_to_string(value.pointer("/choices/0/message/content")?)
}

fn extract_anthropic(value: &Value) -> Option<String> {
    value
        .pointer("/content")?
        .as_array()?
        .iter()
        .find(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .and_then(|block| block.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn content_to_string(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    let parts = content.as_array()?;
    let joined: String = parts
        .iter()
        .filter_map(|part| {
            if let Some(text) = part.as_str() {
                Some(text)
            } else if part.get("type").and_then(Value::as_str) == Some("text") {
                part.get("text").and_then(Value::as_str)
            } else {
                None
            }
        })
        .collect();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn sanitize(text: &str) -> String {
    let trimmed = text.trim();
    let stripped = strip_pair(trimmed, '"', '"')
        .or_else(|| strip_pair(trimmed, '\u{201C}', '\u{201D}'))
        .or_else(|| strip_pair(trimmed, '\'', '\''))
        .or_else(|| strip_pair(trimmed, '\u{2018}', '\u{2019}'))
        .unwrap_or(trimmed);
    stripped.trim().to_string()
}

fn strip_pair(text: &str, open: char, close: char) -> Option<&str> {
    text.strip_prefix(open)?.strip_suffix(close)
}
