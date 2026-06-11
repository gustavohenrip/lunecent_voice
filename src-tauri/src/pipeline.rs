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
    if state.recording.swap(true, Ordering::AcqRel) {
        return;
    }
    let not_ready = {
        let meta = state.engine_meta.read();
        if meta.ready {
            None
        } else {
            Some(match meta.error.as_deref() {
                Some("model not downloaded") => {
                    "Modelo de voz não baixado. Abra os Ajustes para baixá-lo.".to_string()
                }
                Some(other) => format!("Falha no modelo de voz: {other}"),
                None => "O modelo de voz ainda está carregando, aguarde.".to_string(),
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
    state.audio.start();
    state.set_status(Status::Recording);
}

pub fn cancel_recording(state: &SharedState) {
    if !state.recording.swap(false, Ordering::AcqRel) {
        return;
    }
    let _ = state.audio.stop();
    state.set_status(Status::Idle);
}

pub fn finish_recording(app: AppHandle, state: SharedState) {
    if !state.recording.swap(false, Ordering::AcqRel) {
        return;
    }
    if state.busy.swap(true, Ordering::AcqRel) {
        let _ = state.audio.stop();
        let _ = app.emit(
            "pipeline-error",
            PipelineError {
                stage: "busy".to_string(),
                message: "Ainda processando o ditado anterior.".to_string(),
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

    let blocking_state = state.clone();
    let lang_for_blocking = language.clone();
    let join = tokio::task::spawn_blocking(move || {
        let mono = captured.to_mono_16k();
        let peak = mono.iter().fold(0f32, |acc, s| acc.max(s.abs()));
        let mono_len = mono.len();
        let trimmed = maybe_trim(&blocking_state, mono);
        tracing::info!(
            "pipeline audio: mono {} samples (peak {:.4}), after vad {} samples",
            mono_len,
            peak,
            trimmed.len()
        );
        blocking_state.transcribe_blocking(&trimmed, lang_for_blocking.as_deref())
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
    let final_text = if settings.llm_enabled && !local_not_ready {
        state.llm.cleanup(&settings, &processed).await
    } else {
        processed.clone()
    };
    let llm_used = final_text != processed;

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
                    message: format!(
                        "{err}. Janelas elevadas (como administrador) bloqueiam a colagem; o texto está na área de transferência."
                    ),
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

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
