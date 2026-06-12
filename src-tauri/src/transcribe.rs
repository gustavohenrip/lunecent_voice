use crate::error::{AppError, AppResult};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct TranscribeEngine {
    context: WhisperContext,
    pub on_gpu: bool,
}

impl TranscribeEngine {
    pub fn load(model_path: &Path, prefer_gpu: bool) -> AppResult<TranscribeEngine> {
        if !model_path.exists() {
            return Err(AppError::Model(format!(
                "whisper model not found: {}",
                model_path.display()
            )));
        }
        let path_str = model_path.to_string_lossy().to_string();

        if prefer_gpu {
            match Self::try_load(&path_str, true) {
                Ok(context) => {
                    return Ok(TranscribeEngine {
                        context,
                        on_gpu: true,
                    })
                }
                Err(err) => {
                    tracing::warn!("GPU whisper init failed ({err}); falling back to CPU");
                }
            }
        }

        let context = Self::try_load(&path_str, false)?;
        Ok(TranscribeEngine {
            context,
            on_gpu: false,
        })
    }

    fn try_load(path: &str, use_gpu: bool) -> AppResult<WhisperContext> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu(use_gpu);
        WhisperContext::new_with_params(path, params)
            .map_err(|e| AppError::Transcribe(format!("whisper init failed: {e}")))
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        language: Option<&str>,
        n_threads: i32,
        initial_prompt: Option<&str>,
    ) -> AppResult<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        let mut state = self
            .context
            .create_state()
            .map_err(|e| AppError::Transcribe(format!("create state failed: {e}")))?;

        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });
        params.set_n_threads(n_threads.max(1));
        params.set_translate(false);
        params.set_language(language);
        params.set_no_context(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if let Some(prompt) = initial_prompt {
            if !prompt.trim().is_empty() {
                params.set_initial_prompt(prompt);
            }
        }

        state
            .full(params, samples)
            .map_err(|e| AppError::Transcribe(format!("inference failed: {e}")))?;

        let segments = state.full_n_segments();

        let mut text = String::new();
        for index in 0..segments {
            if let Some(segment) = state.get_segment(index) {
                if let Ok(part) = segment.to_str_lossy() {
                    append_segment(&mut text, &part);
                }
            }
        }

        Ok(text.trim().to_string())
    }
}

fn append_segment(out: &mut String, segment: &str) {
    if segment.is_empty() {
        return;
    }
    if out.trim().is_empty() {
        out.push_str(segment);
        return;
    }
    if ends_sentence(out) {
        out.push_str(segment);
    } else {
        out.push_str(&lower_first_alpha(segment));
    }
}

fn ends_sentence(text: &str) -> bool {
    match text.trim_end().chars().last() {
        Some(c) => matches!(c, '.' | '!' | '?' | '\u{2026}' | ':' | '\n'),
        None => true,
    }
}

fn lower_first_alpha(segment: &str) -> String {
    let chars: Vec<char> = segment.chars().collect();
    let mut idx = 0;
    while idx < chars.len() && !chars[idx].is_alphabetic() {
        idx += 1;
    }
    if idx >= chars.len() || !chars[idx].is_uppercase() {
        return segment.to_string();
    }
    let next_is_upper_alpha = chars
        .get(idx + 1)
        .map(|c| c.is_alphabetic() && c.is_uppercase())
        .unwrap_or(false);
    if next_is_upper_alpha {
        return segment.to_string();
    }
    let mut result = String::with_capacity(segment.len());
    for (i, c) in chars.iter().enumerate() {
        if i == idx {
            result.extend(c.to_lowercase());
        } else {
            result.push(*c);
        }
    }
    result
}
