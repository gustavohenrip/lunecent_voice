use crate::error::AppResult;
use crate::state::SharedState;
use tauri::AppHandle;

pub fn apply(_app: &AppHandle, state: &SharedState) -> AppResult<()> {
    #[cfg(windows)]
    crate::inputhook::set_bindings(&state.settings_snapshot());
    #[cfg(not(windows))]
    let _ = state;
    Ok(())
}
