# Lunecent Voice

Hold a hotkey, talk, release. Your speech is transcribed locally and pasted into
whatever window has focus. Runs fully offline. Windows first, with a CPU build for
machines without an NVIDIA GPU.

Transcription is done by whisper.cpp (`large-v3-turbo` by default). There is an
optional local LLM pass (Gemma via llama.cpp) that rewrites the raw transcript, but
it is off by default and you don't need it. The plain Whisper output is already
accurate.

Built with Tauri 2 (Rust) and Svelte 5.

## How it works

```
mic -> cpal capture -> Silero VAD (trim silence) -> whisper.cpp (CUDA or CPU)
    -> filler/dictionary cleanup -> optional LLM rewrite -> clipboard + Ctrl+V
```

- Global hotkey is a low-level Windows keyboard and mouse hook, so it fires even when
  the app is not focused and you can bind mouse buttons (middle, back, forward) on top
  of Ctrl/Shift/Alt/Win.
- The mic stream is kept open and pre-warmed, so recording starts instantly instead of
  waiting for the device to open.
- Whisper tries the GPU first and falls back to CPU on its own. A CPU tag shows on the
  widget when that happens.
- Text is delivered by writing to the clipboard and sending Ctrl+V, then restoring your
  previous clipboard. Elevated windows (admin apps) block simulated input; in that case
  the text is left on the clipboard for you to paste.
- Everything stays on the machine. The network is only touched to download models on
  first run, or if you point the LLM at a remote endpoint.

## Requirements

To run (GPU build):

- Windows 10 64-bit (1809 or newer) or Windows 11.
- NVIDIA GPU with a current driver. About 4 GB VRAM for `large-v3-turbo`, 6 GB+ for
  `large-v3`, plus ~3 GB more if you turn on the Gemma cleanup. An RTX 50-series card
  (Blackwell) needs a driver that ships the CUDA 12.8+ runtime.
- 8 GB system RAM.
- ~5 GB free disk: the app is small, but models download on first run (Whisper turbo is
  1.6 GB, the optional Gemma model is 2.4 GB).
- A microphone.

To run (CPU build):

- Windows 10/11 64-bit, any x64 CPU. No GPU required.
- 8 GB RAM minimum, 16 GB recommended (the model sits in RAM).
- Transcription is much slower than on GPU. `medium` is the practical model here;
  `large-v3-turbo` works but is heavy on CPU.

To build from source, add: Rust (rustup, MSVC toolchain), Node 18+, Visual Studio 2022
with the "Desktop development with C++" workload, CMake, and LLVM (for libclang). The
GPU build also needs the CUDA Toolkit 12.8+ (13.x for RTX 50-series). Budget ~10 GB of
disk for the toolchains and the build output.

## Install (end user)

1. Download the latest installer from the Releases page. There are two: a GPU build
   (NVIDIA only, larger, fast) and a CPU build (runs anywhere, slower).
2. Run the installer.
3. Open Settings, go to Models, and download a Whisper model.
4. Hold Ctrl+Shift+Space, talk, release. The text lands wherever your cursor is.

Hotkeys, model, language, dictionary and the rest are in Settings.

## Build it yourself (Windows)

Install the toolchain once:

```powershell
winget install Rustlang.Rustup Kitware.CMake LLVM.LLVM
winget install Nvidia.CUDA   # GPU build only
```

You also need Visual Studio 2022 with the Desktop C++ workload.

Then, from the project root, double-click `build.bat` for the GPU installer or
`build-cpu.bat` for the universal CPU installer. Both wrap the same steps:

```powershell
. .\scripts\build-env.ps1                 # sets CUDA_PATH, LIBCLANG_PATH, PATH
npm install
npm run fetch-deps                         # GPU build: llama-server + CUDA DLLs + VAD model
npx tauri build                            # GPU
npx tauri build --no-default-features --features vad --config src-tauri/tauri.cpu.conf.json  # CPU
```

Installers land in `src-tauri/target/release/bundle/` (NSIS `.exe` and `.msi`). The
standalone executable is `src-tauri/target/release/lunecent-voice.exe`.

## Run from source (no installer)

Double-click `run-dev.bat`. It kills any running instance, sets up the build
environment, and launches `tauri dev` (Vite on port 1420 plus the Rust app with
hot reload). First run compiles the Rust side, which takes a few minutes.

## Configuration

Settings live in `%APPDATA%\com.lunecent.voice\settings.json` and are editable from the
Settings window. Models and the history database live in the same folder. Notable keys:
`whisper_model`, `language`, `prefer_gpu`, `hotkey_ptt`, `hotkey_toggle`,
`vad_enabled`, `filler_removal`, `dictionary`, and the `llm_*` group for the optional
cleanup pass.

The LLM cleanup is off unless you enable it in Settings and a llama-server binary is
present (the GPU `fetch-deps` pulls one). You can also point `llm_backend` at an
OpenAI-compatible, Anthropic, or Ollama endpoint instead of the local server.

## Source layout

```
src-tauri/src/
  audio.rs        pre-warmed cpal capture, downmix + resample to 16 kHz, mic level
  vad.rs          Silero v5 VAD via onnxruntime, energy-gated fallback
  transcribe.rs   whisper-rs, GPU then CPU
  inputhook.rs    WH_KEYBOARD_LL + WH_MOUSE_LL global hotkey (keyboard and mouse)
  hotkey.rs       binds the hook from settings
  dictionary.rs   filler stripping (non-words only) + exact replacements
  cleanup.rs      optional LLM client (OpenAI-compatible + Anthropic), timeout
  sidecar.rs      llama-server process lifecycle (Job Object, killed on exit)
  services.rs     bootstrap, sidecar restart, autostart
  inject.rs       clipboard save/restore + simulated Ctrl+V
  history.rs      SQLite + FTS5, stats
  models.rs       model registry + downloader with progress
  pipeline.rs     capture -> transcribe -> clean -> inject -> history
  state.rs        shared state
  commands.rs     Tauri command surface
  tray.rs         system tray
  lib.rs          window, tray, DLL path, level emitter setup
src/              Svelte 5 frontend: widget, settings, history windows
scripts/          build-env, fetch-deps, build/run helpers
```

## License

MIT. See [LICENSE](LICENSE).
