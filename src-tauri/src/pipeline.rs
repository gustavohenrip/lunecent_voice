use crate::error::{AppError, AppResult};
use crate::state::{AppState, SharedState, Status};
use crate::{dictionary, history, inject};
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct CompletePayload {
    raw_text: String,
    final_text: String,
    word_count: i64,
    duration_ms: u64,
    on_gpu: bool,
    llm_used: bool,
}

#[derive(Serialize, Clone)]
struct PipelineError {
    stage: String,
    message: String,
}

pub fn begin_recording(state: &SharedState) {
    if state.busy.load(Ordering::Acquire) {
        return;
    }
    if state.recording.swap(true, Ordering::AcqRel) {
        return;
    }
    if state.busy.load(Ordering::Acquire) {
        state.recording.store(false, Ordering::Release);
        return;
    }
    state.cancel.store(false, Ordering::Release);
    let not_ready = {
        let meta = state.engine_meta.read();
        if meta.ready {
            None
        } else {
            Some(match meta.error.as_deref() {
                Some("model not downloaded") => {
                    "Voice model not downloaded. Open Settings to download it.".to_string()
                }
                Some(other) => format!("Voice model failure: {other}"),
                None => "The voice model is still loading, please wait.".to_string(),
            })
        }
    };
    if let Some(message) = not_ready {
        state.recording.store(false, Ordering::Release);
        let _ = state.app.emit(
            "pipeline-error",
            PipelineError {
                stage: "engine".to_string(),
                message,
            },
        );
        return;
    }
    crate::sound::play(true);
    state.audio.start();
    state.set_status(Status::Recording);
}

pub fn cancel_recording(state: &SharedState) {
    state.cancel.store(true, Ordering::Release);
    if state.recording.swap(false, Ordering::AcqRel) {
        let _ = state.audio.stop();
        state.set_status(Status::Idle);
    }
}

pub fn finish_recording(app: AppHandle, state: SharedState) {
    if !state.recording.swap(false, Ordering::AcqRel) {
        return;
    }
    crate::sound::play(false);
    if state.busy.swap(true, Ordering::AcqRel) {
        let _ = state.audio.stop();
        let _ = app.emit(
            "pipeline-error",
            PipelineError {
                stage: "busy".to_string(),
                message: "Still processing the previous dictation.".to_string(),
            },
        );
        return;
    }

    let captured = state.audio.stop();
    state.set_status(Status::Processing);

    tauri::async_runtime::spawn(async move {
        if let Err(err) = run_pipeline(&app, &state, captured).await {
            tracing::error!("pipeline error: {err}");
            let _ = app.emit(
                "pipeline-error",
                PipelineError {
                    stage: "pipeline".to_string(),
                    message: err.to_string(),
                },
            );
        }
        state.busy.store(false, Ordering::Release);
        state.set_status(Status::Idle);
    });
}

pub fn toggle_recording(app: AppHandle, state: SharedState) {
    if state.recording.load(Ordering::Acquire) {
        finish_recording(app, state);
    } else if state.busy.load(Ordering::Acquire) {
        cancel_recording(&state);
    } else {
        begin_recording(&state);
    }
}

async fn run_pipeline(
    app: &AppHandle,
    state: &SharedState,
    captured: crate::audio::CapturedAudio,
) -> AppResult<()> {
    let settings = state.settings_snapshot();
    let duration_ms = captured.duration_ms;
    let language = settings.language_code();
    let whisper_translated = settings.whisper_translate();

    let blocking_state = state.clone();
    let lang_for_blocking = language.clone();
    let join = tokio::task::spawn_blocking(move || {
        if blocking_state.cancel.load(Ordering::Acquire) {
            return Ok((String::new(), false));
        }
        let mono = captured.to_mono_16k();
        let peak = mono.iter().fold(0f32, |acc, s| acc.max(s.abs()));
        let mono_len = mono.len();
        let vad_start = std::time::Instant::now();
        let trimmed = maybe_trim(&blocking_state, mono);
        tracing::info!(
            "pipeline audio: mono {} samples (peak {:.4}), after vad {} samples, vad took {} ms",
            mono_len,
            peak,
            trimmed.len(),
            vad_start.elapsed().as_millis()
        );
        if blocking_state.cancel.load(Ordering::Acquire) {
            return Ok((String::new(), false));
        }
        let whisper_start = std::time::Instant::now();
        let result = blocking_state.transcribe_blocking(
            &trimmed,
            lang_for_blocking.as_deref(),
            whisper_translated,
        );
        tracing::info!("whisper took {} ms", whisper_start.elapsed().as_millis());
        result
    })
    .await;

    let (raw, on_gpu) = match join {
        Ok(Ok(value)) => value,
        Ok(Err(err)) => return Err(err),
        Err(join_err) => {
            if join_err.is_panic() {
                tracing::error!("transcription panicked; reloading whisper on CPU");
                let recovery = state.clone();
                let _ = tokio::task::spawn_blocking(move || recovery.load_engine(false)).await;
            }
            return Err(AppError::Transcribe(
                "transcription crashed; switched to CPU mode".to_string(),
            ));
        }
    };

    tracing::info!("whisper returned {} chars", raw.len());

    if state.cancel.load(Ordering::Acquire) {
        let _ = app.emit("transcription-cancelled", ());
        return Ok(());
    }

    if raw.trim().is_empty() {
        let _ = app.emit("transcription-empty", ());
        return Ok(());
    }

    let processed = dictionary::process(
        &raw,
        settings.filler_removal,
        &settings.filler_words,
        &settings.dictionary,
    );

    let local_not_ready = settings.llm_backend == crate::config::LlmBackend::Local
        && !state.sidecar_ready.load(Ordering::Acquire);
    let want_llm = settings.llm_enabled || (settings.translation_enabled && !whisper_translated);
    let final_text = if want_llm && !local_not_ready {
        let eff: std::borrow::Cow<'_, crate::config::Settings> = if whisper_translated {
            let mut tuned = settings.clone();
            tuned.translation_enabled = false;
            std::borrow::Cow::Owned(tuned)
        } else {
            std::borrow::Cow::Borrowed(&settings)
        };
        let llm_start = std::time::Instant::now();
        let cleanup = state.llm.cleanup(&*eff, &processed);
        tokio::pin!(cleanup);
        let text = loop {
            tokio::select! {
                out = &mut cleanup => break out,
                _ = tokio::time::sleep(std::time::Duration::from_millis(120)) => {
                    if state.cancel.load(Ordering::Acquire) {
                        let _ = app.emit("transcription-cancelled", ());
                        return Ok(());
                    }
                }
            }
        };
        tracing::info!("llm cleanup took {} ms", llm_start.elapsed().as_millis());
        text
    } else {
        if want_llm && local_not_ready {
            tracing::info!("llm skipped: local server not ready");
        }
        processed.clone()
    };
    let llm_used = final_text != processed;

    if state.cancel.load(Ordering::Acquire) {
        let _ = app.emit("transcription-cancelled", ());
        return Ok(());
    }

    let inject_text = final_text.clone();
    let restore = settings.restore_clipboard;
    let delay = settings.paste_delay_ms;
    let inject_result =
        tokio::task::spawn_blocking(move || inject::inject_text(&inject_text, restore, delay)).await;

    match inject_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            tracing::warn!("injection failed: {err}");
            let _ = app.emit(
                "pipeline-error",
                PipelineError {
                    stage: "inject".to_string(),
                    message: format!("{err}. {}", inject_block_hint()),
                },
            );
        }
        Err(_) => {
            let _ = inject::copy_to_clipboard(&final_text);
        }
    }

    let entry = history::NewEntry {
        created_at: now_millis(),
        duration_ms: duration_ms as i64,
        word_count: dictionary::word_count(&final_text),
        raw_text: raw.clone(),
        final_text: final_text.clone(),
        language: settings.language.clone(),
        on_gpu,
        llm_used,
    };
    let _ = state.db_insert(&entry);

    let _ = app.emit(
        "transcription-complete",
        CompletePayload {
            raw_text: raw,
            final_text,
            word_count: entry.word_count,
            duration_ms,
            on_gpu,
            llm_used,
        },
    );

    Ok(())
}

fn maybe_trim(state: &AppState, mono: Vec<f32>) -> Vec<f32> {
    let settings = state.settings_snapshot();
    if !settings.vad_enabled {
        return mono;
    }
    let guard = state.vad.read();
    match guard.as_ref() {
        Some(vad) => vad.trim(
            &mono,
            settings.vad_threshold,
            settings.min_silence_ms,
            settings.speech_pad_ms,
        ),
        None => mono,
    }
}

fn inject_block_hint() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Grant Accessibility permission in System Settings to paste automatically; the text is on the clipboard."
    }
    #[cfg(not(target_os = "macos"))]
    {
        "Elevated windows (run as administrator) block pasting; the text is on the clipboard."
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
