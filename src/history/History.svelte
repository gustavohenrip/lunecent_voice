<script lang="ts">
  import { onMount } from "svelte";
  import { api, on, getCurrentWindow, type UnlistenFn } from "../lib/ipc";
  import type { HistoryEntry, Stats } from "../lib/types";

  let entries = $state<HistoryEntry[]>([]);
  let stats = $state<Stats>({ total_entries: 0, total_words: 0, total_speaking_ms: 0, avg_wpm: 0 });
  let query = $state("");
  let expanded = $state<Set<number>>(new Set());
  let copiedId = $state<number>(0);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    let unlisten: UnlistenFn[] = [];
    refresh();
    (async () => {
      try {
        unlisten.push(await on("transcription-complete", () => refresh()));
      } catch (_) {}
    })();
    return () => {
      clearTimeout(searchTimer);
      unlisten.forEach((u) => u());
    };
  });

  function refresh() {
    api.getStats().then((s) => (stats = s)).catch(() => {});
    if (query.trim()) {
      api.searchHistory(query.trim(), 200).then((e) => (entries = e)).catch(() => {});
    } else {
      api.getHistory(200, 0).then((e) => (entries = e)).catch(() => {});
    }
  }

  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(refresh, 180);
  }

  async function copy(id: number) {
    try {
      await api.recopy(id);
      copiedId = id;
      setTimeout(() => (copiedId = 0), 1200);
    } catch (_) {}
  }

  async function remove(id: number) {
    await api.deleteHistory(id).catch(() => {});
    refresh();
  }

  async function clearAll() {
    if (confirm("Apagar todo o histórico?")) {
      await api.clearHistory().catch(() => {});
      refresh();
    }
  }

  function toggle(id: number) {
    const next = new Set(expanded);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expanded = next;
  }

  async function toDict(entry: HistoryEntry) {
    const phrase = prompt("Frase falada (como o Whisper ouviu):", entry.raw_text);
    if (phrase === null) return;
    const replacement = prompt("Substituição exata:", entry.final_text);
    if (replacement === null) return;
    await api.addToDictionary(phrase, replacement).catch(() => {});
  }

  function fmtTime(ms: number): string {
    return new Date(ms).toLocaleString("pt-BR", {
      day: "2-digit",
      month: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function fmtDuration(ms: number): string {
    return (ms / 1000).toFixed(1) + " s";
  }

  function fmtSpeaking(ms: number): string {
    const min = Math.floor(ms / 60000);
    if (min >= 60) return (min / 60).toFixed(1) + " h";
    if (min >= 1) return min + " min";
    return Math.round(ms / 1000) + " s";
  }

  function wpm(entry: HistoryEntry): number {
    if (entry.duration_ms <= 0) return 0;
    return Math.round((entry.word_count * 60000) / entry.duration_ms);
  }

  function minimize() {
    getCurrentWindow().minimize().catch(() => {});
  }
</script>

<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand">
      <span class="logo"></span>
      <h1>Lunecent Voice <span>· Histórico e estatísticas</span></h1>
    </div>
    <div class="head-actions">
      <button class="btn danger" onclick={clearAll}>Limpar tudo</button>
      <div class="winbtns">
        <button class="winbtn" title="Minimizar" aria-label="Minimizar" onclick={minimize}>
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M5 12h14" stroke-linecap="round" />
          </svg>
        </button>
        <button class="winbtn close" title="Fechar" aria-label="Fechar" onclick={() => api.hideWindow("history")}>
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M6 6l12 12M18 6L6 18" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    </div>
  </header>

  <div class="stats">
    <div class="card stat">
      <span class="num">{stats.total_words.toLocaleString("pt-BR")}</span>
      <span class="lbl">palavras</span>
    </div>
    <div class="card stat">
      <span class="num">{fmtSpeaking(stats.total_speaking_ms)}</span>
      <span class="lbl">tempo falando</span>
    </div>
    <div class="card stat">
      <span class="num">{stats.avg_wpm.toFixed(0)}</span>
      <span class="lbl" title="Palavras por minuto">média de PPM</span>
    </div>
    <div class="card stat">
      <span class="num">{stats.total_entries.toLocaleString("pt-BR")}</span>
      <span class="lbl">ditados</span>
    </div>
  </div>

  <div class="searchbar">
    <input placeholder="Buscar no histórico…" bind:value={query} oninput={onSearch} />
  </div>

  <div class="list">
    {#each entries as e (e.id)}
      <div class="card entry">
        <div
          class="entry-main"
          onclick={() => toggle(e.id)}
          onkeydown={(ev) => {
            if (ev.key === "Enter" || ev.key === " ") toggle(e.id);
          }}
          role="button"
          tabindex="0"
        >
          <div class="final">{e.final_text}</div>
          {#if expanded.has(e.id) && e.raw_text !== e.final_text}
            <div class="raw">original: {e.raw_text}</div>
          {/if}
          <div class="meta">
            <span>{fmtTime(e.created_at)}</span>
            <span>·</span>
            <span>{fmtDuration(e.duration_ms)}</span>
            <span>·</span>
            <span title="Palavras por minuto">{wpm(e)} PPM</span>
            {#if e.on_gpu}<span class="tag gpu">GPU</span>{:else}<span class="tag cpu">CPU</span>{/if}
            {#if e.llm_used}<span class="tag llm">IA</span>{/if}
          </div>
        </div>
        <div class="entry-actions">
          <button class="btn ghost" onclick={() => copy(e.id)}>
            {copiedId === e.id ? "Copiado" : "Copiar"}
          </button>
          <button class="btn ghost" onclick={() => toDict(e)}>Dicionário</button>
          <button class="btn danger" onclick={() => remove(e.id)}>Excluir</button>
        </div>
      </div>
    {/each}
    {#if entries.length === 0}
      <p class="empty">Nenhum ditado ainda. Segure o atalho e fale.</p>
    {/if}
  </div>
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

  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
    padding: 16px 20px 4px;
  }

  .stat {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .num {
    font-size: 22px;
    font-weight: 700;
    background: linear-gradient(90deg, #d6d9ff, #aab0ff);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .lbl {
    font-size: 11px;
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .searchbar {
    padding: 12px 20px;
  }

  .searchbar input {
    width: 100%;
  }

  .list {
    flex: 1;
    overflow-y: auto;
    padding: 0 20px 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .entry {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 13px 15px;
    transition: border-color 0.16s ease, background 0.16s ease;
  }

  .entry:hover {
    border-color: var(--stroke-strong);
    background: var(--glass-strong);
  }

  .entry-main {
    flex: 1;
    min-width: 0;
    cursor: pointer;
  }

  .final {
    font-size: 14px;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }

  .raw {
    margin-top: 6px;
    font-size: 12px;
    color: var(--text-faint);
    font-style: italic;
    overflow-wrap: anywhere;
  }

  .meta {
    margin-top: 8px;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 7px;
    font-size: 11px;
    color: var(--text-faint);
  }

  .tag {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 999px;
    letter-spacing: 0.4px;
  }

  .tag.gpu {
    background: var(--good-soft);
    color: var(--good);
  }

  .tag.cpu {
    background: var(--warn-soft);
    color: var(--warn);
  }

  .tag.llm {
    background: var(--accent-soft);
    color: var(--accent-text);
  }

  .entry-actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 0 0 auto;
    opacity: 0;
    transition: opacity 0.18s ease;
  }

  .entry:hover .entry-actions,
  .entry:focus-within .entry-actions {
    opacity: 1;
  }

  .empty {
    text-align: center;
    color: var(--text-faint);
    padding: 40px;
  }
</style>
