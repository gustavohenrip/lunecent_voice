<script lang="ts">
  import { onMount } from "svelte";
  import { api, on, getCurrentWindow, type UnlistenFn } from "../lib/ipc";
  import type { Settings, ModelStatus, DownloadProgress } from "../lib/types";

  type Tab = "general" | "audio" | "models" | "llm" | "dictionary";

  let settings = $state<Settings | null>(null);
  let devices = $state<string[]>([]);
  let models = $state<ModelStatus[]>([]);
  let downloads = $state<Record<string, DownloadProgress>>({});
  let tab = $state<Tab>("general");
  let saving = $state(false);
  let saved = $state(false);
  let savedTimer: ReturnType<typeof setTimeout> | undefined;
  let capturing = $state<null | "hotkey_ptt" | "hotkey_toggle">(null);
  let llmTest = $state<string>("");
  let dictKey = $state("");
  let dictVal = $state("");
  let fillerText = $state("");

  onMount(() => {
    let unlisten: UnlistenFn[] = [];
    reload();
    (async () => {
      try {
        unlisten.push(
          await on<DownloadProgress>("model-download-progress", (e) => {
            downloads = { ...downloads, [e.payload.id]: e.payload };
            if (e.payload.done) refreshModels();
          }),
        );
      } catch (_) {}
    })();
    return () => {
      clearTimeout(savedTimer);
      unlisten.forEach((u) => u());
    };
  });

  async function reload() {
    try {
      settings = await api.getSettings();
      fillerText = settings.filler_words.join("\n");
    } catch (_) {}
    api.listAudioDevices().then((d) => (devices = d)).catch(() => {});
    refreshModels();
  }

  function refreshModels() {
    api.modelStatuses().then((m) => (models = m)).catch(() => {});
  }

  async function save() {
    if (!settings) return;
    saving = true;
    try {
      settings.filler_words = fillerText
        .split(/[\n,]/)
        .map((s) => s.trim())
        .filter(Boolean);
      await api.saveSettings($state.snapshot(settings) as Settings);
      saved = true;
      clearTimeout(savedTimer);
      savedTimer = setTimeout(() => (saved = false), 2400);
    } catch (e) {
      llmTest = "Falha ao salvar: " + e;
    } finally {
      saving = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (!capturing || !settings) return;
    e.preventDefault();
    if (e.code === "Escape") {
      capturing = null;
      return;
    }
    const accel = accelFromKeyboard(e);
    if (accel) {
      settings[capturing] = accel;
      capturing = null;
    }
  }

  function onMouse(e: MouseEvent) {
    if (!capturing || !settings) return;
    const token = mouseToken(e.button);
    if (!token) {
      if (e.button === 0) {
        const target = e.target as HTMLElement | null;
        if (!target || !target.closest("button")) capturing = null;
      }
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    settings[capturing] = [...modsFrom(e), token].join("+");
    capturing = null;
  }

  function mouseToken(button: number): string | null {
    switch (button) {
      case 1:
        return "MouseMiddle";
      case 3:
        return "MouseBack";
      case 4:
        return "MouseForward";
      default:
        return null;
    }
  }

  function modsFrom(e: KeyboardEvent | MouseEvent): string[] {
    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.shiftKey) mods.push("Shift");
    if (e.altKey) mods.push("Alt");
    if (e.metaKey) mods.push("Super");
    return mods;
  }

  function accelFromKeyboard(e: KeyboardEvent): string | null {
    const key = normalizeKey(e.code);
    if (!key) return null;
    return [...modsFrom(e), key].join("+");
  }

  function normalizeKey(code: string): string | null {
    const modifiers = [
      "ControlLeft",
      "ControlRight",
      "ShiftLeft",
      "ShiftRight",
      "AltLeft",
      "AltRight",
      "MetaLeft",
      "MetaRight",
    ];
    if (modifiers.includes(code)) return null;
    if (code === "Space") return "Space";
    if (code.startsWith("Key")) return code.slice(3);
    if (code.startsWith("Digit")) return code.slice(5);
    if (/^F\d{1,2}$/.test(code)) return code;
    const map: Record<string, string> = {
      Backquote: "`",
      Minus: "-",
      Equal: "=",
      Comma: ",",
      Period: ".",
      Slash: "/",
      Semicolon: ";",
      Quote: "'",
      BracketLeft: "[",
      BracketRight: "]",
      Backslash: "\\",
      Enter: "Enter",
      Tab: "Tab",
      ArrowUp: "Up",
      ArrowDown: "Down",
      ArrowLeft: "Left",
      ArrowRight: "Right",
    };
    return map[code] ?? null;
  }

  function prettyHotkey(accel: string): string {
    if (!accel) return "Nenhum";
    return accel
      .split("+")
      .map((part) => {
        switch (part) {
          case "MouseMiddle":
            return "Mouse 3 (meio)";
          case "MouseBack":
            return "Mouse 4 (lateral)";
          case "MouseForward":
            return "Mouse 5 (lateral)";
          case "Super":
            return "Win";
          default:
            return part;
        }
      })
      .join(" + ");
  }

  function download(id: string) {
    api.downloadModel(id).catch(() => {});
  }

  async function testLlm() {
    llmTest = "Testando…";
    try {
      const out = await api.testLlm();
      llmTest = "OK: " + out.slice(0, 80);
    } catch (e) {
      llmTest = "Erro: " + e;
    }
  }

  function addDict() {
    if (!settings || !dictKey.trim()) return;
    settings.dictionary = { ...settings.dictionary, [dictKey.trim()]: dictVal };
    dictKey = "";
    dictVal = "";
  }

  function removeDict(key: string) {
    if (!settings) return;
    const next = { ...settings.dictionary };
    delete next[key];
    settings.dictionary = next;
  }

  function fmtBytes(n: number): string {
    if (n <= 0) return "0";
    const gb = n / 1_073_741_824;
    if (gb >= 1) return gb.toFixed(2) + " GB";
    return (n / 1_048_576).toFixed(0) + " MB";
  }

  function minimize() {
    getCurrentWindow().minimize().catch(() => {});
  }

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "general", label: "Geral", icon: "M12 3l8 5v8l-8 5-8-5V8z" },
    { id: "audio", label: "Áudio", icon: "M12 3v18M7 8v8M17 8v8M3 11v2M21 11v2" },
    { id: "models", label: "Modelos", icon: "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" },
    { id: "llm", label: "LLM", icon: "M12 2a7 7 0 0 1 7 7c0 2.4-1.2 4.1-2.6 5.4-.8.8-1.4 1.9-1.4 3.1V19a2 2 0 0 1-2 2h-2a2 2 0 0 1-2-2v-1.5c0-1.2-.6-2.3-1.4-3.1C5.2 13.1 4 11.4 4 9a7 7 0 0 1 8-7z" },
    { id: "dictionary", label: "Dicionário", icon: "M4 19.5A2.5 2.5 0 0 1 6.5 17H20V2H6.5A2.5 2.5 0 0 0 4 4.5zM4 19.5A2.5 2.5 0 0 0 6.5 22H20v-5" },
  ];
</script>

<svelte:window onkeydown={onKey} onmousedown={onMouse} />

<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand">
      <span class="logo"></span>
      <h1>Lunecent Voice <span>· Ajustes</span></h1>
    </div>
    <div class="head-actions">
      {#if saved}<span class="saved-chip">Salvo</span>{/if}
      <button class="btn primary" onclick={save} disabled={saving}>
        {saving ? "Salvando…" : "Salvar"}
      </button>
      <div class="winbtns">
        <button class="winbtn" title="Minimizar" aria-label="Minimizar" onclick={minimize}>
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M5 12h14" stroke-linecap="round" />
          </svg>
        </button>
        <button class="winbtn close" title="Fechar" aria-label="Fechar" onclick={() => api.hideWindow("settings")}>
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M6 6l12 12M18 6L6 18" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    </div>
  </header>

  {#if settings}
    <div class="body">
      <nav>
        {#each tabs as t}
          <button class="tab" class:active={tab === t.id} onclick={() => (tab = t.id)}>
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
              <path d={t.icon} />
            </svg>
            {t.label}
          </button>
        {/each}
      </nav>

      <section class="panel">
        {#if tab === "general"}
          <div class="card group">
            <h2>Atalhos</h2>
            <div class="field">
              <label for="ptt">Segurar para falar</label>
              <div class="hotkey">
                <input id="ptt" readonly value={capturing === "hotkey_ptt" ? "Pressione teclas ou botão do mouse…" : prettyHotkey(settings.hotkey_ptt)} class:capturing={capturing === "hotkey_ptt"} />
                <button class="btn" onclick={() => (capturing = capturing === "hotkey_ptt" ? null : "hotkey_ptt")}>
                  {capturing === "hotkey_ptt" ? "Cancelar" : "Definir"}
                </button>
              </div>
            </div>
            <div class="field">
              <label for="tog">Alternar gravação (liga/desliga)</label>
              <div class="hotkey">
                <input id="tog" readonly value={capturing === "hotkey_toggle" ? "Pressione teclas ou botão do mouse…" : prettyHotkey(settings.hotkey_toggle)} class:capturing={capturing === "hotkey_toggle"} />
                <button class="btn" onclick={() => (capturing = capturing === "hotkey_toggle" ? null : "hotkey_toggle")}>
                  {capturing === "hotkey_toggle" ? "Cancelar" : "Definir"}
                </button>
              </div>
            </div>
            <p class="hint">
              Aceita combinações com Ctrl, Shift, Alt e Win, teclas comuns e os botões do mouse:
              meio (Mouse 3) e laterais (Mouse 4 e 5). Pressione Esc para cancelar a captura.
            </p>
            <div class="field">
              <label for="mode">Modo de gravação</label>
              <select id="mode" bind:value={settings.record_mode}>
                <option value="push_to_talk">Segurar para falar</option>
                <option value="toggle">Alternar (pressionar para iniciar e parar)</option>
              </select>
            </div>
          </div>

          <div class="card group">
            <h2>Idioma e modelo</h2>
            <div class="field">
              <label for="lang">Idioma da fala</label>
              <select id="lang" bind:value={settings.language}>
                <option value="auto">Detectar automaticamente</option>
                <option value="pt">Português (pt-BR)</option>
                <option value="en">Inglês (en)</option>
              </select>
            </div>
            <div class="field">
              <label for="wm">Modelo Whisper</label>
              <select id="wm" bind:value={settings.whisper_model}>
                <option value="large-v3-turbo">large-v3-turbo (recomendado)</option>
                <option value="large-v3">large-v3 (máxima precisão)</option>
                <option value="medium">medium (mais leve)</option>
              </select>
            </div>
            <label class="switch">
              <input type="checkbox" bind:checked={settings.prefer_gpu} />
              <span class="track"><span class="thumb"></span></span>
              <span>Usar GPU (CUDA) quando disponível</span>
            </label>
          </div>

          <div class="card group">
            <h2>Comportamento</h2>
            <label class="switch">
              <input type="checkbox" bind:checked={settings.restore_clipboard} />
              <span class="track"><span class="thumb"></span></span>
              <span>Restaurar a área de transferência após colar</span>
            </label>
            <label class="switch">
              <input type="checkbox" bind:checked={settings.autostart} />
              <span class="track"><span class="thumb"></span></span>
              <span>Iniciar junto com o Windows</span>
            </label>
            <div class="field">
              <label for="delay">Atraso ao colar: {settings.paste_delay_ms} ms</label>
              <input id="delay" type="range" min="40" max="400" step="10" bind:value={settings.paste_delay_ms} />
            </div>
          </div>
        {/if}

        {#if tab === "audio"}
          <div class="card group">
            <h2>Entrada de áudio</h2>
            <div class="field">
              <label for="dev">Microfone</label>
              <select id="dev" bind:value={settings.audio_device}>
                <option value={null}>Padrão do sistema</option>
                {#each devices as d}
                  <option value={d}>{d}</option>
                {/each}
              </select>
            </div>
          </div>

          <div class="card group">
            <h2>Detecção de fala (VAD)</h2>
            <label class="switch">
              <input type="checkbox" bind:checked={settings.vad_enabled} />
              <span class="track"><span class="thumb"></span></span>
              <span>Cortar silêncio automaticamente (Silero VAD)</span>
            </label>
            <div class="field">
              <label for="thr">Sensibilidade: {settings.vad_threshold.toFixed(2)}</label>
              <input id="thr" type="range" min="0.1" max="0.9" step="0.05" bind:value={settings.vad_threshold} />
            </div>
            <div class="field">
              <label for="pad">Folga antes e depois da fala: {settings.speech_pad_ms} ms</label>
              <input id="pad" type="range" min="0" max="400" step="10" bind:value={settings.speech_pad_ms} />
            </div>
            <div class="field">
              <label for="sil">Silêncio mínimo entre trechos: {settings.min_silence_ms} ms</label>
              <input id="sil" type="range" min="50" max="1000" step="50" bind:value={settings.min_silence_ms} />
            </div>
          </div>

          <div class="card group">
            <h2>Limpeza do texto</h2>
            <label class="switch">
              <input type="checkbox" bind:checked={settings.filler_removal} />
              <span class="track"><span class="thumb"></span></span>
              <span>Remover muletas de fala (né, tipo, hum…)</span>
            </label>
            <div class="field">
              <label for="fill">Palavras a remover (uma por linha)</label>
              <textarea id="fill" rows="5" bind:value={fillerText}></textarea>
            </div>
          </div>
        {/if}

        {#if tab === "models"}
          <p class="hint">
            Os modelos são baixados do Hugging Face e ficam armazenados no seu computador.
            Nada é enviado para a internet além do próprio download.
          </p>
          {#each models as m}
            <div class="card model">
              <div class="model-info">
                <div class="model-name">{m.info.label}</div>
                <div class="model-meta">
                  {m.info.filename} · {fmtBytes(m.info.size_bytes)}
                  {#if m.present}<span class="ok">instalado</span>{/if}
                </div>
                {#if downloads[m.info.id] && !downloads[m.info.id].done && !downloads[m.info.id].error}
                  <div class="bar"><span style={`width:${downloads[m.info.id].pct}%`}></span></div>
                  <div class="pct">{downloads[m.info.id].pct.toFixed(1)}%</div>
                {/if}
                {#if downloads[m.info.id]?.error}
                  <div class="err-text">{downloads[m.info.id].error}</div>
                {/if}
              </div>
              <button class="btn" onclick={() => download(m.info.id)} disabled={!!downloads[m.info.id] && !downloads[m.info.id].done && !downloads[m.info.id].error}>
                {m.present ? "Baixar novamente" : "Baixar"}
              </button>
            </div>
          {/each}
          <div>
            <button class="btn" onclick={() => api.reloadEngine()}>Recarregar modelo de voz</button>
          </div>
        {/if}

        {#if tab === "llm"}
          <div class="card group">
            <h2>Correção com IA</h2>
            <label class="switch">
              <input type="checkbox" bind:checked={settings.llm_enabled} />
              <span class="track"><span class="thumb"></span></span>
              <span>Corrigir a fala com IA (reescreve o texto como você quis dizer)</span>
            </label>
            <div class="field">
              <label for="bk">Backend</label>
              <select id="bk" bind:value={settings.llm_backend}>
                <option value="local">Local (Gemma via llama-server)</option>
                <option value="open_ai_compatible">Compatível com OpenAI</option>
                <option value="anthropic">Anthropic</option>
                <option value="ollama">Ollama</option>
              </select>
            </div>
            {#if settings.llm_backend === "local"}
              <div class="field">
                <label for="lm">Modelo local</label>
                <select id="lm" bind:value={settings.llm_local_model}>
                  {#each models.filter((m) => m.info.kind === "llm") as m}
                    <option value={m.info.filename}>{m.info.label}</option>
                  {/each}
                </select>
              </div>
              <div>
                <button class="btn" onclick={() => api.restartLlm()}>Reiniciar servidor local</button>
              </div>
            {/if}
          </div>

          <div class="card group">
            <h2>Conexão</h2>
            <div class="field">
              <label for="ep">Endpoint</label>
              <input id="ep" bind:value={settings.llm_endpoint} />
            </div>
            <div class="field">
              <label for="mn">Nome do modelo</label>
              <input id="mn" bind:value={settings.llm_model_name} />
            </div>
            {#if settings.llm_backend !== "local"}
              <div class="field">
                <label for="key">Chave de API</label>
                <input id="key" type="password" bind:value={settings.llm_api_key} />
              </div>
            {/if}
            <div class="field">
              <label for="to">Tempo limite: {settings.llm_timeout_ms} ms</label>
              <input id="to" type="range" min="500" max="8000" step="100" bind:value={settings.llm_timeout_ms} />
            </div>
            <div class="field">
              <label for="tmp">Temperatura: {settings.llm_temperature.toFixed(2)}</label>
              <input id="tmp" type="range" min="0" max="1" step="0.05" bind:value={settings.llm_temperature} />
            </div>
            <div class="row">
              <button class="btn" onclick={testLlm}>Testar conexão</button>
              {#if llmTest}<span class="hint">{llmTest}</span>{/if}
            </div>
          </div>
        {/if}

        {#if tab === "dictionary"}
          <p class="hint">
            Substituições exatas aplicadas ao texto reconhecido, ideais para nomes próprios,
            termos técnicos e siglas.
          </p>
          <div class="card group">
            <div class="dict-add">
              <input placeholder="como é falado" bind:value={dictKey} />
              <span class="arrow">→</span>
              <input placeholder="substituição exata" bind:value={dictVal} />
              <button class="btn primary" onclick={addDict}>Adicionar</button>
            </div>
          </div>
          <div class="dict-list">
            {#each Object.entries(settings.dictionary) as [k, v]}
              <div class="card dict-row">
                <span class="k">{k}</span>
                <span class="arrow">→</span>
                <span class="v">{v}</span>
                <button class="btn danger" onclick={() => removeDict(k)}>Remover</button>
              </div>
            {/each}
            {#if Object.keys(settings.dictionary).length === 0}
              <p class="hint">Nenhuma entrada ainda.</p>
            {/if}
          </div>
        {/if}
      </section>
    </div>
  {:else}
    <div class="loading">Carregando…</div>
  {/if}
</div>

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .saved-chip {
    color: var(--good);
    background: var(--good-soft);
    border: 1px solid rgba(111, 220, 174, 0.3);
    font-size: 12px;
    font-weight: 600;
    padding: 4px 10px;
    border-radius: 999px;
    animation: chip-in 0.2s ease;
  }

  @keyframes chip-in {
    from {
      opacity: 0;
      transform: scale(0.9);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  nav {
    width: 190px;
    flex: 0 0 auto;
    padding: 14px 10px;
    border-right: 1px solid var(--stroke);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 10px;
    text-align: left;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    font-weight: 600;
    font-size: 13px;
    color: var(--text-soft);
    border: 1px solid transparent;
    transition: background 0.16s ease, color 0.16s ease, border-color 0.16s ease;
  }

  .tab:hover {
    background: var(--glass);
    color: var(--text);
  }

  .tab.active {
    background: var(--accent-soft);
    border-color: rgba(140, 150, 255, 0.30);
    color: var(--accent-text);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
  }

  .tab:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .panel {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .group {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .group h2 {
    margin: 0;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    color: var(--text-faint);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field label {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-soft);
  }

  .hotkey {
    display: flex;
    gap: 8px;
  }

  .hotkey input {
    flex: 1;
  }

  .hotkey input.capturing {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    animation: capture-glow 1.2s ease-in-out infinite;
  }

  @keyframes capture-glow {
    0%,
    100% {
      box-shadow: 0 0 0 3px var(--accent-soft);
    }
    50% {
      box-shadow: 0 0 0 5px rgba(140, 150, 255, 0.26);
    }
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 11px;
    font-size: 13.5px;
    cursor: pointer;
    color: var(--text);
  }

  .switch input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch .track {
    width: 38px;
    height: 22px;
    flex: 0 0 auto;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.10);
    border: 1px solid var(--stroke);
    padding: 2px;
    display: flex;
    align-items: center;
    transition: background 0.2s ease, border-color 0.2s ease;
  }

  .switch .thumb {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
    transform: translateX(0);
    transition: transform 0.2s ease;
  }

  .switch input:checked + .track {
    background: linear-gradient(135deg, rgba(140, 150, 255, 0.9), rgba(120, 110, 245, 0.9));
    border-color: rgba(170, 178, 255, 0.5);
  }

  .switch input:checked + .track .thumb {
    transform: translateX(16px);
  }

  .switch input:focus-visible + .track {
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  .row .hint {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-faint);
    margin: 0;
  }

  .model {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 14px 16px;
  }

  .model-info {
    min-width: 0;
    flex: 1;
  }

  .model-name {
    font-weight: 600;
    font-size: 13.5px;
  }

  .model-meta {
    font-size: 11.5px;
    color: var(--text-faint);
    margin-top: 3px;
  }

  .ok {
    color: var(--good);
    font-weight: 600;
    margin-left: 6px;
  }

  .bar {
    margin-top: 9px;
    height: 6px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--accent-deep), var(--accent-2));
    transition: width 0.25s ease;
  }

  .pct {
    margin-top: 5px;
    font-size: 11px;
    color: var(--text-faint);
  }

  .err-text {
    color: var(--danger);
    font-size: 11.5px;
    margin-top: 6px;
  }

  .dict-add {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dict-add input {
    flex: 1;
    min-width: 0;
  }

  .arrow {
    color: var(--text-faint);
    flex: 0 0 auto;
  }

  .dict-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .dict-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 14px;
  }

  .dict-row .k {
    font-weight: 600;
  }

  .dict-row .v {
    flex: 1;
    color: var(--text-soft);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .loading {
    display: grid;
    place-items: center;
    flex: 1;
    color: var(--text-faint);
  }
</style>
