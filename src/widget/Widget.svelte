<script lang="ts">
  import { onMount } from "svelte";
  import { api, on, getCurrentWindow, type UnlistenFn } from "../lib/ipc";
  import { currentMonitor } from "@tauri-apps/api/window";
  import type { StatusPayload, CompletePayload, PipelineErrorPayload } from "../lib/types";

  const BAR_COUNT = 18;
  const PILL_HALF = 69;
  const DOCK_NEED = 70;

  let status = $state<StatusPayload>({
    status: "idle",
    recording: false,
    cpu_mode: false,
    engine_ready: false,
    model: "",
    audio_available: true,
    vad_active: false,
    error: null,
  });

  let levels = $state<number[]>(Array(BAR_COUNT).fill(0));
  let flash = $state<{ text: string; kind: "ok" | "err" } | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;
  let dockSide = $state<"left" | "right">("right");

  const idleText = $derived.by(() => {
    if (!status.audio_available) return "Sem microfone";
    if (status.status === "loading") return "Carregando…";
    if (status.status === "error") return "Erro no modelo";
    if (!status.engine_ready) return "Baixe o modelo";
    return "Lunecent Voice";
  });

  function showFlash(text: string, kind: "ok" | "err") {
    if (!text) return;
    flash = { text: text.length > 30 ? text.slice(0, 29) + "…" : text, kind };
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash = null), kind === "err" ? 4500 : 2600);
  }

  function pushLevel(raw: number) {
    const shaped = Math.min(1, Math.pow(Math.max(0, raw) * 6.5, 0.75));
    const next = levels.slice(1);
    next.push(shaped);
    levels = next;
  }

  function resetLevels() {
    levels = Array(BAR_COUNT).fill(0);
  }

  async function updateDockSide() {
    try {
      const win = getCurrentWindow();
      const [pos, size, scale, mon] = await Promise.all([
        win.outerPosition(),
        win.outerSize(),
        win.scaleFactor(),
        currentMonitor(),
      ]);
      if (!mon) return;
      const pillCenterX = pos.x + size.width / 2;
      const pillRightX = pillCenterX + PILL_HALF * scale;
      const monitorRight = mon.position.x + mon.size.width;
      const need = DOCK_NEED * scale;
      dockSide = monitorRight - pillRightX >= need ? "right" : "left";
    } catch (_) {}
  }

  onMount(() => {
    let unlisten: UnlistenFn[] = [];
    api.getStatus().then((s) => (status = s)).catch(() => {});
    updateDockSide();
    (async () => {
      try {
        unlisten.push(await getCurrentWindow().onMoved(() => updateDockSide()));
        unlisten.push(await getCurrentWindow().onResized(() => updateDockSide()));
        unlisten.push(await getCurrentWindow().onScaleChanged(() => updateDockSide()));
        unlisten.push(
          await on<StatusPayload>("status-changed", (e) => {
            const was = status.status;
            status = e.payload;
            if (status.status === "recording" && was !== "recording") resetLevels();
          }),
        );
        unlisten.push(await on<number>("audio-level", (e) => pushLevel(e.payload)));
        unlisten.push(
          await on<CompletePayload>("transcription-complete", (e) =>
            showFlash(e.payload.final_text, "ok"),
          ),
        );
        unlisten.push(
          await on<PipelineErrorPayload | string>("pipeline-error", (e) =>
            showFlash(typeof e.payload === "string" ? e.payload : e.payload.message, "err"),
          ),
        );
        unlisten.push(
          await on("transcription-empty", () => showFlash("Nenhuma fala detectada", "err")),
        );
      } catch (_) {}
    })();
    return () => {
      clearTimeout(flashTimer);
      unlisten.forEach((u) => u());
    };
  });

  function toggle() {
    api.toggleRecording().catch(() => {});
  }
</script>

{#snippet dockButtons()}
  <button class="icon" aria-label="Ajustes" onclick={() => api.openWindow("settings")}>
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <line x1="4" y1="8" x2="14" y2="8" />
      <line x1="18" y1="8" x2="20" y2="8" />
      <line x1="4" y1="16" x2="10" y2="16" />
      <line x1="14" y1="16" x2="20" y2="16" />
      <circle cx="16" cy="8" r="2" />
      <circle cx="12" cy="16" r="2" />
    </svg>
  </button>
  <button class="icon" aria-label="Histórico" onclick={() => api.openWindow("history")}>
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
      <path d="M12 7v5l4 2" />
    </svg>
  </button>
{/snippet}

<div class="stage">
  <div class="dock left" class:active={dockSide === "left"}>
    {@render dockButtons()}
  </div>

  <div class="pill" data-state={status.status} data-tauri-drag-region>
    <span class="sheen"></span>

    <button
      class="orb"
      data-state={status.status}
      onclick={toggle}
      aria-label={status.status === "recording" ? "Parar gravação" : "Iniciar gravação"}
    >
      <span class="core"></span>
      <span class="halo"></span>
    </button>

    <div class="center" data-tauri-drag-region>
      {#if status.status === "recording"}
        <div class="wave" aria-label="Nível do microfone">
          {#each levels as value}
            <span class="bar" style={`height:${(2 + value * 14).toFixed(1)}px`}></span>
          {/each}
        </div>
      {:else if flash}
        <span class="flash" class:err={flash.kind === "err"}>{flash.text}</span>
      {:else if status.status === "processing"}
        <span class="brand shimmer">Transcrevendo…</span>
      {:else}
        <span class="brand" class:dim={!status.engine_ready || !status.audio_available}>
          {idleText}
        </span>
      {/if}
    </div>
  </div>

  <div class="dock right" class:active={dockSide === "right"}>
    {@render dockButtons()}
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
    user-select: none;
  }

  .stage {
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 2px;
  }

  .dock {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 0 0 auto;
    width: 57px;
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 0.24s ease,
      transform 0.42s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .dock.left {
    justify-content: flex-end;
    transform: translateX(14px);
  }

  .dock.right {
    justify-content: flex-start;
    transform: translateX(-14px);
  }

  .stage:hover .dock.active {
    opacity: 1;
    transform: translateX(0);
    pointer-events: auto;
  }

  .stage:hover .dock.active .icon:nth-child(2) {
    transition-delay: 0.05s;
  }

  .pill {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
    width: 138px;
    height: 28px;
    padding: 0 9px 0 4px;
    border-radius: 14px;
    overflow: hidden;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.16), rgba(255, 255, 255, 0.03) 48%, rgba(255, 255, 255, 0.07)),
      linear-gradient(135deg, rgba(38, 40, 48, 0.72), rgba(18, 19, 24, 0.78));
    border: 1px solid rgba(255, 255, 255, 0.22);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.32),
      inset 0 -1px 1px rgba(0, 0, 0, 0.28),
      inset 1px 0 0 rgba(255, 255, 255, 0.07),
      inset -1px 0 0 rgba(255, 255, 255, 0.07);
    color: #f4f5f8;
    transition: border-color 0.25s ease;
  }

  .sheen {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 46%;
    border-radius: 14px 14px 40% 40%;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.16), rgba(255, 255, 255, 0.015));
    pointer-events: none;
  }

  .pill::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 14px;
    background: linear-gradient(115deg, transparent 30%, rgba(255, 255, 255, 0.10) 46%, rgba(255, 255, 255, 0.02) 54%, transparent 70%);
    background-size: 240% 100%;
    background-position: 120% 0;
    pointer-events: none;
    transition: background-position 0.9s ease;
  }

  .pill:hover::after {
    background-position: -120% 0;
  }

  .pill[data-state="recording"] {
    border-color: rgba(255, 120, 130, 0.45);
  }

  .orb {
    position: relative;
    width: 22px;
    height: 22px;
    flex: 0 0 auto;
    border-radius: 50%;
    z-index: 1;
  }

  .core {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background:
      radial-gradient(circle at 32% 26%, rgba(255, 255, 255, 0.95), rgba(255, 255, 255, 0.18) 46%),
      linear-gradient(150deg, #e9ecf2, #9aa0ad 70%, #767c89);
    box-shadow:
      inset 0 -1px 2px rgba(0, 0, 0, 0.25),
      0 1px 4px rgba(0, 0, 0, 0.35);
    transition: background 0.3s ease, box-shadow 0.3s ease, transform 0.25s ease;
  }

  .halo {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 1.5px solid transparent;
    box-sizing: border-box;
    transition: border-color 0.3s ease;
  }

  .orb:hover .core {
    transform: scale(1.08);
  }

  .orb[data-state="recording"] .core {
    background:
      radial-gradient(circle at 32% 26%, rgba(255, 255, 255, 0.9), rgba(255, 255, 255, 0.1) 46%),
      linear-gradient(150deg, #ff8d96, #e74c5e 70%, #c63a4d);
    box-shadow:
      inset 0 -1px 2px rgba(0, 0, 0, 0.3),
      0 1px 5px rgba(190, 40, 60, 0.4);
  }

  .orb[data-state="recording"] .halo {
    border-color: rgba(240, 90, 105, 0.5);
    animation: pulse 1.4s ease-out infinite;
  }

  .orb[data-state="processing"] .halo {
    border-color: transparent;
    border-top-color: rgba(255, 255, 255, 0.75);
    animation: spin 0.85s linear infinite;
  }

  .orb[data-state="loading"] .core {
    animation: breathe 1.5s ease-in-out infinite;
  }

  .orb[data-state="error"] .core {
    background:
      radial-gradient(circle at 32% 26%, rgba(255, 255, 255, 0.85), rgba(255, 255, 255, 0.1) 46%),
      linear-gradient(150deg, #f0a3a3, #c96a6a);
  }

  @keyframes pulse {
    0% {
      transform: scale(0.85);
      opacity: 0.9;
    }
    100% {
      transform: scale(1.3);
      opacity: 0;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes breathe {
    0%,
    100% {
      transform: scale(0.85);
      opacity: 0.6;
    }
    50% {
      transform: scale(1.05);
      opacity: 1;
    }
  }

  .center {
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    cursor: grab;
    overflow: hidden;
    z-index: 1;
  }

  .center:active {
    cursor: grabbing;
  }

  .brand {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: rgba(248, 249, 252, 0.92);
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }

  .brand.dim {
    color: rgba(240, 242, 248, 0.55);
    font-weight: 500;
    font-size: 10px;
  }

  .brand.shimmer {
    background: linear-gradient(
      90deg,
      rgba(244, 245, 248, 0.45) 20%,
      #ffffff 40%,
      rgba(244, 245, 248, 0.45) 60%
    );
    background-size: 220% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    text-shadow: none;
    animation: shimmer 1.5s linear infinite;
  }

  @keyframes shimmer {
    from {
      background-position: 120% 0;
    }
    to {
      background-position: -120% 0;
    }
  }

  .flash {
    font-size: 10px;
    font-weight: 500;
    color: #b9f0d4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    pointer-events: none;
    animation: fadein 0.22s ease;
  }

  .flash.err {
    color: #ffacb8;
  }

  @keyframes fadein {
    from {
      opacity: 0;
      transform: translateY(2px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .wave {
    display: flex;
    align-items: center;
    gap: 1.5px;
    height: 18px;
    width: 100%;
    pointer-events: none;
  }

  .bar {
    flex: 1 1 auto;
    min-height: 2px;
    max-width: 3px;
    border-radius: 1.5px;
    background: rgba(248, 249, 252, 0.88);
    transition: height 60ms ease-out;
  }

  .icon {
    width: 26px;
    height: 26px;
    flex: 0 0 auto;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: rgba(244, 245, 248, 0.9);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.14), rgba(255, 255, 255, 0.03)),
      linear-gradient(135deg, rgba(40, 42, 50, 0.72), rgba(20, 21, 26, 0.78));
    border: 1px solid rgba(255, 255, 255, 0.20);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.28),
      0 3px 8px rgba(0, 0, 0, 0.32);
    transition: background 0.16s ease, color 0.16s ease, transform 0.14s ease;
  }

  .icon:hover {
    color: #fff;
    transform: translateY(-1px);
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.22), rgba(255, 255, 255, 0.06)),
      linear-gradient(135deg, rgba(52, 54, 64, 0.78), rgba(28, 29, 36, 0.82));
  }

  .icon:active {
    transform: translateY(0);
  }

  .icon:focus-visible {
    outline: none;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.28),
      0 0 0 2px rgba(255, 255, 255, 0.4);
  }

  @media (prefers-reduced-motion: reduce) {
    .pill::after,
    .icon,
    .bar,
    .dock {
      transition: none;
    }
    .halo,
    .core,
    .brand.shimmer {
      animation: none !important;
    }
  }
</style>
