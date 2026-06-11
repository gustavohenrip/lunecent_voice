<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { api, on, getCurrentWindow, type UnlistenFn } from "../lib/ipc";
  import type { Settings, ModelStatus, DownloadProgress } from "../lib/types";
  import Icon from "../lib/Icon.svelte";
  import { getTheme, setTheme, type ThemeMode } from "../lib/theme";

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
  let themeMode = $state<ThemeMode>(getTheme());

  const reduce =
    typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  const isMac =
    typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);
  const capturePrompt = isMac
    ? "Pressione as teclas…"
    : "Pressione teclas ou botão do mouse…";

  const themes: { id: ThemeMode; label: string; icon: string }[] = [
    { id: "light", label: "Claro", icon: "sun" },
    { id: "system", label: "Sistema", icon: "monitor" },
    { id: "dark", label: "Escuro", icon: "moon-stars" },
  ];

  function pickTheme(mode: ThemeMode) {
    themeMode = mode;
    setTheme(mode);
  }

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
    const token = isMac ? null : mouseToken(e.button);
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
            return isMac ? "Cmd" : "Win";
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
    { id: "general", label: "Geral", icon: "sliders-horizontal" },
    { id: "audio", label: "Áudio", icon: "waveform" },
    { id: "models", label: "Modelos", icon: "cube" },
    { id: "llm", label: "Correção IA", icon: "sparkle" },
    { id: "dictionary", label: "Dicionário", icon: "book-open-text" },
  ];

  const active = $derived(tabs.find((t) => t.id === tab) ?? tabs[0]);
</script>

<svelte:window onkeydown={onKey} onmousedown={onMouse} />

<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand">
      <span class="logo"></span>
      <h1>Lunecent Voice <span>· Ajustes</span></h1>
    </div>
    <div class="head-actions">
      {#if saved}
        <span class="saved-chip"><Icon name="check-circle" size={15} /> Salvo</span>
      {/if}
      <button class="btn primary" onclick={save} disabled={saving}>
        {saving ? "Salvando…" : "Salvar"}
      </button>
      <div class="winbtns">
        <button class="winbtn" title="Minimizar" aria-label="Minimizar" onclick={minimize}>
          <Icon name="minus" size={15} />
        </button>
        <button class="winbtn close" title="Fechar" aria-label="Fechar" onclick={() => api.hideWindow("settings")}>
          <Icon name="x" size={15} />
        </button>
      </div>
    </div>
  </header>

  {#if settings}
    <div class="body">
      <nav>
        {#each tabs as t}
          <button class="nav-item" class:active={tab === t.id} onclick={() => (tab = t.id)}>
            <Icon name={t.icon} size={18} />
            <span>{t.label}</span>
          </button>
        {/each}
      </nav>

      <section class="panel">
        {#key tab}
          <div class="panel-inner" in:fly={{ y: 6, duration: reduce ? 0 : 260, easing: cubicOut }}>
            <h2 class="panel-title display">{active.label}</h2>

            {#if tab === "general"}
              <div class="group">
                <span class="group-head">Atalhos</span>
                <div class="field">
                  <label for="ptt">Segurar para falar</label>
                  <div class="hotkey">
                    <input id="ptt" readonly value={capturing === "hotkey_ptt" ? capturePrompt : prettyHotkey(settings.hotkey_ptt)} class:capturing={capturing === "hotkey_ptt"} />
                    <button class="btn" onclick={() => (capturing = capturing === "hotkey_ptt" ? null : "hotkey_ptt")}>
                      {capturing === "hotkey_ptt" ? "Cancelar" : "Definir"}
                    </button>
                  </div>
                </div>
                <div class="field">
                  <label for="tog">Alternar gravação (liga/desliga)</label>
                  <div class="hotkey">
                    <input id="tog" readonly value={capturing === "hotkey_toggle" ? capturePrompt : prettyHotkey(settings.hotkey_toggle)} class:capturing={capturing === "hotkey_toggle"} />
                    <button class="btn" onclick={() => (capturing = capturing === "hotkey_toggle" ? null : "hotkey_toggle")}>
                      {capturing === "hotkey_toggle" ? "Cancelar" : "Definir"}
                    </button>
                  </div>
                </div>
                <p class="hint">
                  {#if isMac}
                    Aceita combinações com Ctrl, Shift, Alt (Option) e Cmd e teclas comuns.
                    Pressione Esc para cancelar a captura. No macOS, conceda a permissão de
                    Acessibilidade ao app para o atalho global funcionar.
                  {:else}
                    Aceita combinações com Ctrl, Shift, Alt e Win, teclas comuns e os botões do mouse:
                    meio (Mouse 3) e laterais (Mouse 4 e 5). Pressione Esc para cancelar a captura.
                  {/if}
                </p>
                <div class="field">
                  <label for="mode">Modo de gravação</label>
                  <select id="mode" bind:value={settings.record_mode}>
                    <option value="push_to_talk">Segurar para falar</option>
                    <option value="toggle">Alternar (pressionar para iniciar e parar)</option>
                  </select>
                </div>
              </div>

              <div class="group">
                <span class="group-head">Idioma e modelo</span>
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
                  <span>{isMac ? "Usar GPU (Metal) quando disponível" : "Usar GPU (CUDA) quando disponível"}</span>
                </label>
              </div>

              <div class="group">
                <span class="group-head">Comportamento</span>
                <label class="switch">
                  <input type="checkbox" bind:checked={settings.restore_clipboard} />
                  <span class="track"><span class="thumb"></span></span>
                  <span>Restaurar a área de transferência após colar</span>
                </label>
                <label class="switch">
                  <input type="checkbox" bind:checked={settings.autostart} />
                  <span class="track"><span class="thumb"></span></span>
                  <span>{isMac ? "Iniciar junto com o macOS" : "Iniciar junto com o Windows"}</span>
                </label>
                <div class="field">
                  <label for="delay">Atraso ao colar <em class="tnum">{settings.paste_delay_ms} ms</em></label>
                  <input id="delay" type="range" min="40" max="400" step="10" bind:value={settings.paste_delay_ms} />
                </div>
              </div>

              <div class="group">
                <span class="group-head">Aparência</span>
                <div class="field">
                  <span class="cap">Tema</span>
                  <div class="theme-seg" role="group" aria-label="Tema">
                    {#each themes as th}
                      <button class="seg" class:active={themeMode === th.id} onclick={() => pickTheme(th.id)}>
                        <Icon name={th.icon} size={15} />
                        <span>{th.label}</span>
                      </button>
                    {/each}
                  </div>
                </div>
              </div>
            {/if}

            {#if tab === "audio"}
              <div class="group">
                <span class="group-head">Entrada de áudio</span>
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

              <div class="group">
                <span class="group-head">Detecção de fala (VAD)</span>
                <label class="switch">
                  <input type="checkbox" bind:checked={settings.vad_enabled} />
                  <span class="track"><span class="thumb"></span></span>
                  <span>Cortar silêncio automaticamente (Silero VAD)</span>
                </label>
                <div class="field">
                  <label for="thr">Sensibilidade <em class="tnum">{settings.vad_threshold.toFixed(2)}</em></label>
                  <input id="thr" type="range" min="0.1" max="0.9" step="0.05" bind:value={settings.vad_threshold} />
                </div>
                <div class="field">
                  <label for="pad">Folga antes e depois da fala <em class="tnum">{settings.speech_pad_ms} ms</em></label>
                  <input id="pad" type="range" min="0" max="400" step="10" bind:value={settings.speech_pad_ms} />
                </div>
                <div class="field">
                  <label for="sil">Silêncio mínimo entre trechos <em class="tnum">{settings.min_silence_ms} ms</em></label>
                  <input id="sil" type="range" min="50" max="1000" step="50" bind:value={settings.min_silence_ms} />
                </div>
              </div>

              <div class="group">
                <span class="group-head">Limpeza do texto</span>
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
              <p class="hint lead">Os modelos são baixados do Hugging Face e ficam armazenados no seu
                computador. Nada é enviado para a internet além do próprio download.</p>
              <div class="group">
                {#each models as m, i}
                  <div class="model" class:divided={i > 0}>
                    <div class="model-info">
                      <div class="model-name">{m.info.label}</div>
                      <div class="model-meta tnum">
                        {m.info.filename} · {fmtBytes(m.info.size_bytes)}
                        {#if m.present}<span class="ok"><Icon name="check-circle" size={14} /> instalado</span>{/if}
                      </div>
                      {#if downloads[m.info.id] && !downloads[m.info.id].done && !downloads[m.info.id].error}
                        <div class="bar"><span style={`width:${downloads[m.info.id].pct}%`}></span></div>
                        <div class="pct tnum">{downloads[m.info.id].pct.toFixed(1)}%</div>
                      {/if}
                      {#if downloads[m.info.id]?.error}
                        <div class="err-text">{downloads[m.info.id].error}</div>
                      {/if}
                    </div>
                    <button class="btn" onclick={() => download(m.info.id)} disabled={!!downloads[m.info.id] && !downloads[m.info.id].done && !downloads[m.info.id].error}>
                      {#if downloads[m.info.id] && !downloads[m.info.id].done && !downloads[m.info.id].error}
                        Baixando…
                      {:else}
                        <Icon name="download-simple" size={15} />
                        {m.present ? "Baixar de novo" : "Baixar"}
                      {/if}
                    </button>
                  </div>
                {/each}
              </div>
              <div class="afoot">
                <button class="btn" onclick={() => api.reloadEngine()}>Recarregar modelo de voz</button>
              </div>
            {/if}

            {#if tab === "llm"}
              <div class="group">
                <span class="group-head">Correção com IA</span>
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
                  <div class="afoot">
                    <button class="btn" onclick={() => api.restartLlm()}>Reiniciar servidor local</button>
                  </div>
                {/if}
              </div>

              <div class="group">
                <span class="group-head">Conexão</span>
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
                  <label for="to">Tempo limite <em class="tnum">{settings.llm_timeout_ms} ms</em></label>
                  <input id="to" type="range" min="500" max="8000" step="100" bind:value={settings.llm_timeout_ms} />
                </div>
                <div class="field">
                  <label for="tmp">Temperatura <em class="tnum">{settings.llm_temperature.toFixed(2)}</em></label>
                  <input id="tmp" type="range" min="0" max="1" step="0.05" bind:value={settings.llm_temperature} />
                </div>
                <div class="row">
                  <button class="btn" onclick={testLlm}>Testar conexão</button>
                  {#if llmTest}<span class="hint">{llmTest}</span>{/if}
                </div>
              </div>
            {/if}

            {#if tab === "dictionary"}
              <p class="hint lead">Substituições exatas aplicadas ao texto reconhecido, ideais para nomes
                próprios, termos técnicos e siglas.</p>
              <div class="dict-add">
                <input placeholder="como é falado" bind:value={dictKey} />
                <span class="arrow"><Icon name="arrow-right" size={16} /></span>
                <input placeholder="substituição exata" bind:value={dictVal} />
                <button class="btn primary" onclick={addDict}>Adicionar</button>
              </div>
              <div class="group">
                {#each Object.entries(settings.dictionary) as [k, v], i}
                  <div class="dict-row" class:divided={i > 0}>
                    <span class="k">{k}</span>
                    <span class="arrow"><Icon name="arrow-right" size={15} /></span>
                    <span class="v">{v}</span>
                    <button class="btn ghost danger" onclick={() => removeDict(k)}>Remover</button>
                  </div>
                {/each}
                {#if Object.keys(settings.dictionary).length === 0}
                  <div class="empty">
                    <Icon name="book-open-text" size={26} />
                    <p>Nenhuma entrada ainda.</p>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/key}
      </section>
    </div>
  {:else}
    <div class="loading">Carregando…</div>
  {/if}
</div>

<style>
  .shell {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--paper);
    border: 1px solid var(--paper-edge);
    border-radius: 12px;
    overflow: hidden;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .saved-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--sage-text);
    font-size: 13px;
    font-weight: 600;
    animation: chip-in 0.24s ease;
  }

  @keyframes chip-in {
    from {
      opacity: 0;
      transform: translateX(6px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  nav {
    width: 218px;
    flex: 0 0 auto;
    padding: 18px 14px;
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    text-align: left;
    padding: 10px 13px;
    border-radius: var(--radius-sm);
    font-family: var(--font-body);
    font-weight: 500;
    font-size: 14.5px;
    color: var(--ink-soft);
    transition: background 0.16s ease, color 0.16s ease;
  }

  .nav-item :global(.ph) {
    color: var(--ink-faint);
    transition: color 0.16s ease;
  }

  .nav-item:hover {
    background: var(--paper-sunk);
    color: var(--ink);
  }

  .nav-item.active {
    background: var(--terra-soft);
    color: var(--terra-text);
    font-weight: 600;
  }

  .nav-item.active :global(.ph) {
    color: var(--terra);
  }

  .nav-item:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--terra-soft);
  }

  .panel {
    flex: 1;
    overflow-y: auto;
    padding: 28px 34px 36px;
  }

  .panel-inner {
    max-width: 600px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .panel-title {
    margin: 0 0 6px;
    font-size: var(--t-display);
  }

  .group {
    padding: 18px 20px;
    background: var(--paper-raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    gap: 15px;
  }

  .group-head {
    font-family: var(--font-body);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .field label,
  .field .cap {
    font-family: var(--font-body);
    font-size: 13.5px;
    font-weight: 600;
    color: var(--ink-soft);
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  .field label em {
    font-style: normal;
    color: var(--terra-text);
    font-weight: 600;
    margin-left: auto;
  }

  .field select,
  .field > input,
  .field textarea {
    max-width: 440px;
  }

  .hotkey {
    display: flex;
    gap: 8px;
    max-width: 440px;
  }

  .hotkey input {
    flex: 1;
    font-feature-settings: "tnum" 1;
  }

  .hotkey input.capturing {
    border-color: var(--terra);
    box-shadow: 0 0 0 3px var(--terra-soft);
    animation: capture-glow 1.3s ease-in-out infinite;
  }

  @keyframes capture-glow {
    0%,
    100% {
      box-shadow: 0 0 0 3px var(--terra-soft);
    }
    50% {
      box-shadow: 0 0 0 5px var(--terra-glow);
    }
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 13px;
    font-size: 14.5px;
    cursor: pointer;
    color: var(--ink);
  }

  .switch input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch .track {
    width: 40px;
    height: 23px;
    flex: 0 0 auto;
    border-radius: 999px;
    background: var(--paper-sunk);
    border: 1px solid var(--line-strong);
    padding: 2px;
    display: flex;
    align-items: center;
    transition: background 0.2s ease, border-color 0.2s ease;
  }

  .switch .thumb {
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: #faf9f5;
    box-shadow: var(--shadow-sm);
    transform: translateX(0);
    transition: transform 0.2s cubic-bezier(0.3, 0.7, 0.4, 1);
  }

  .switch input:checked + .track {
    background: var(--terra);
    border-color: var(--terra);
  }

  .switch input:checked + .track .thumb {
    transform: translateX(17px);
  }

  .switch input:focus-visible + .track {
    box-shadow: 0 0 0 3px var(--terra-soft);
  }

  .theme-seg {
    display: inline-flex;
    gap: 3px;
    background: var(--paper-sunk);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 3px;
  }

  .seg {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border-radius: 7px;
    font-family: var(--font-body);
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-faint);
    transition: background 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
  }

  .seg:hover {
    color: var(--ink);
  }

  .seg.active {
    background: var(--field);
    color: var(--terra-text);
    box-shadow: var(--shadow-sm);
  }

  .seg.active :global(.ph) {
    color: var(--terra);
  }

  .seg:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--terra-soft);
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

  .afoot {
    margin-top: 2px;
  }

  .hint {
    font-size: 13.5px;
    line-height: 1.6;
    color: var(--ink-faint);
    margin: 0;
  }

  .hint.lead {
    max-width: 520px;
    margin-bottom: 4px;
    color: var(--ink-soft);
  }

  .model {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 4px 0;
  }

  .model.divided {
    border-top: 1px solid var(--line);
    padding-top: 16px;
    margin-top: 1px;
  }

  .model-info {
    min-width: 0;
    flex: 1;
  }

  .model-name {
    font-family: var(--font-display);
    font-size: var(--t-title);
    font-weight: 600;
    color: var(--ink);
  }

  .model-meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 12.5px;
    color: var(--ink-faint);
    margin-top: 4px;
  }

  .ok {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--sage-text);
    font-weight: 600;
  }

  .bar {
    margin-top: 11px;
    height: 5px;
    max-width: 360px;
    background: var(--paper-sunk);
    border-radius: 5px;
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    border-radius: 5px;
    background: var(--terra);
    transition: width 0.3s ease;
  }

  .pct {
    margin-top: 6px;
    font-size: 12px;
    color: var(--ink-faint);
  }

  .err-text {
    color: var(--danger);
    font-size: 13px;
    margin-top: 7px;
  }

  .dict-add {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 600px;
  }

  .dict-add input {
    flex: 1;
    min-width: 0;
  }

  .arrow {
    color: var(--ink-faint);
    flex: 0 0 auto;
    display: inline-flex;
  }

  .dict-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 4px 0;
  }

  .dict-row.divided {
    border-top: 1px solid var(--line);
    padding-top: 12px;
    margin-top: 1px;
  }

  .dict-row .k {
    font-weight: 600;
    color: var(--ink);
  }

  .dict-row .v {
    flex: 1;
    color: var(--ink-soft);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 26px;
    color: var(--ink-ghost);
  }

  .empty p {
    margin: 0;
    font-size: 14px;
    color: var(--ink-faint);
  }

  .loading {
    display: grid;
    place-items: center;
    flex: 1;
    color: var(--ink-faint);
    font-size: 16px;
  }

  @media (max-width: 680px) {
    nav {
      width: 188px;
    }
    .panel {
      padding: 22px 22px 30px;
    }
  }
</style>
