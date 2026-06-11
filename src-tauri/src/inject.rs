use crate::error::{AppError, AppResult};
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::time::Duration;

pub fn inject_text(text: &str, restore_clipboard: bool, paste_delay_ms: u64) -> AppResult<()> {
    if text.trim().is_empty() {
        return Ok(());
    }

    let mut clipboard =
        Clipboard::new().map_err(|e| AppError::Inject(format!("clipboard open failed: {e}")))?;

    let previous = clipboard.get_text().ok();

    set_clipboard_with_retry(&mut clipboard, text)?;

    send_paste()?;

    std::thread::sleep(Duration::from_millis(paste_delay_ms.clamp(40, 1000)));

    if restore_clipboard {
        if let Some(prev) = previous {
            std::thread::sleep(Duration::from_millis(200));
            let _ = set_clipboard_with_retry(&mut clipboard, &prev);
        }
    }

    Ok(())
}

fn set_clipboard_with_retry(clipboard: &mut Clipboard, text: &str) -> AppResult<()> {
    let mut last_err = String::new();
    for attempt in 0..5 {
        match clipboard.set_text(text.to_owned()) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = err.to_string();
                std::thread::sleep(Duration::from_millis(15 * (attempt + 1)));
            }
        }
    }
    Err(AppError::Inject(format!(
        "clipboard write failed after retries: {last_err}"
    )))
}

fn send_paste() -> AppResult<()> {
    let mut enigo = Enigo::new(&EnigoSettings::default())
        .map_err(|e| AppError::Inject(format!("input backend init failed: {e}")))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| AppError::Inject(format!("ctrl press failed: {e}")))?;

    let paste_result = enigo.key(Key::Unicode('v'), Direction::Click);

    let release_result = enigo.key(Key::Control, Direction::Release);

    paste_result.map_err(|e| AppError::Inject(format!("paste key failed: {e}")))?;
    release_result.map_err(|e| AppError::Inject(format!("ctrl release failed: {e}")))?;

    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> AppResult<()> {
    let mut clipboard =
        Clipboard::new().map_err(|e| AppError::Inject(format!("clipboard open failed: {e}")))?;
    set_clipboard_with_retry(&mut clipboard, text)
}
