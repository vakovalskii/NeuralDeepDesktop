import { useEffect, useRef, useState } from "react";
import {
  getHealth, getAgentInfo, getSubscription, getUsage, getSessions, getSessionMessages,
  ensureSession, streamChat, pickWorkspace, generateImage, getWorkspaceDiff, getSkills,
  saveImage, openImage, applyConfig, setSandbox,
  deleteSession, renameSession, generateTitle, copyText,
  backendStatus, provision, restartApp,
  speak, stopSpeak, transcribe, isTauri,
  type Health, type AgentInfo, type Subscription, type Usage, type SessionRow, type DiffFile, type SkillRow,
} from "./transport";
import { Md } from "./Md";
import { Icon } from "./Icon";

const PIN_KEY = "nd.pinnedSessions";
const loadPins = (): string[] => {
  try { return JSON.parse(localStorage.getItem(PIN_KEY) || "[]"); } catch { return []; }
};
const TITLE_KEY = "nd.chatTitles";
const loadTitles = (): Record<string, string> => {
  try { return JSON.parse(localStorage.getItem(TITLE_KEY) || "{}"); } catch { return {}; }
};

type Role = "user" | "assistant";
interface Tool { name: string; status: string }
interface Msg {
  role: Role;
  content: string;
  reasoning?: string;
  tools?: Tool[];
  usage?: any;
  pending?: boolean;
  image?: string;
}

const cap = (s?: string) => (s ? s.charAt(0).toUpperCase() + s.slice(1) : "—");
const fmtK = (n?: number) => (n == null ? "?" : n >= 1000 ? `${(n / 1000).toFixed(n >= 10000 ? 0 : 1)}k` : `${n}`);

// Render a unified-diff patch with per-line coloring.
function Patch({ text }: { text: string }) {
  const lines = text.split("\n").filter(
    (l) => !/^(diff --git|index |--- |\+\+\+ |new file mode|deleted file mode)/.test(l)
  );
  return (
    <pre className="diff-pre">
      {lines.map((l, i) => {
        const cls = l.startsWith("@@") ? "hunk" : l.startsWith("+") ? "add" : l.startsWith("-") ? "del" : "ctx";
        return <div key={i} className={`dl ${cls}`}>{l || " "}</div>;
      })}
    </pre>
  );
}

const SUGGESTIONS = [
  { icon: "check", label: "Проверить free-тариф", prompt: "Reply with exactly: NEURALDEEP_FREE_TIER_OK" },
  { icon: "brain", label: "Показать reasoning (17×23)", prompt: "Сколько будет 17*23? Думай пошагово, потом ответь." },
  { icon: "puzzle", label: "Что ты умеешь?", prompt: "Кратко: кто ты, какая модель отвечает и какие у тебя возможности?" },
  { icon: "image", label: "Сгенерить картинку", prompt: "/img green neon ND logo on pure black, minimal" },
] as const;

// Local slash-commands (merged with Hermes skills in the in-chat menu).
const LOCAL_CMDS = [
  { name: "img", icon: "image", hint: "сгенерировать картинку — /img <промпт>" },
  { name: "new", icon: "plus", hint: "новый чат" },
  { name: "diff", icon: "search", hint: "показать дифф рабочей папки" },
  { name: "folder", icon: "folder", hint: "сменить рабочую папку агента" },
] as const;

export function App() {
  const [messages, setMessages] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [health, setHealth] = useState<Health | null>(null);
  const [agent, setAgent] = useState<AgentInfo | null>(null);
  const [sub, setSub] = useState<Subscription | null>(null);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [showReasoning, setShowReasoning] = useState(true);
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [pins, setPins] = useState<string[]>(loadPins);
  const [titles, setTitles] = useState<Record<string, string>>(loadTitles);
  const [search, setSearch] = useState("");
  const [skills, setSkills] = useState<SkillRow[]>([]);
  const [diffs, setDiffs] = useState<DiffFile[]>([]);
  const [diffOpen, setDiffOpen] = useState(true);
  const [ctxUsed, setCtxUsed] = useState(0);
  const [restarting, setRestarting] = useState(false);
  const [del, setDel] = useState<SessionRow | null>(null);
  const [ren, setRen] = useState<{ s: SessionRow; value: string } | null>(null);
  const [copied, setCopied] = useState<number | null>(null);
  const [speaking, setSpeaking] = useState<number | null>(null);
  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [boot, setBoot] = useState<"checking" | "needs" | "provisioning" | "ready">("checking");
  const [provStage, setProvStage] = useState("download");
  const [provLog, setProvLog] = useState<string[]>([]);
  const [provError, setProvError] = useState<string | null>(null);
  const sessionRef = useRef<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const stickRef = useRef(true); // stick to bottom unless the user scrolled up
  const mediaRecRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const audioCtxRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const rafRef = useRef<number | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const waveCanvasRef = useRef<HTMLCanvasElement | null>(null);

  const refreshSessions = () => getSessions().then((s) => setSessions(s.slice(0, 40)));

  function togglePin(id: string, e: React.MouseEvent) {
    e.stopPropagation();
    setPins((prev) => {
      const next = prev.includes(id) ? prev.filter((x) => x !== id) : [id, ...prev];
      localStorage.setItem(PIN_KEY, JSON.stringify(next));
      return next;
    });
  }

  function saveTitle(id: string, title: string) {
    setTitles((prev) => {
      const next = { ...prev, [id]: title };
      localStorage.setItem(TITLE_KEY, JSON.stringify(next));
      return next;
    });
  }
  const titleOf = (s: SessionRow) => titles[s.id] || s.title || s.id.slice(0, 22);

  function renameChat(s: SessionRow, e: React.MouseEvent) {
    e.stopPropagation();
    setRen({ s, value: titleOf(s) });
  }
  function delChat(s: SessionRow, e: React.MouseEvent) {
    e.stopPropagation();
    setDel(s);
  }

  function commitRename() {
    if (!ren || !ren.value.trim()) { setRen(null); return; }
    const { s, value } = ren;
    saveTitle(s.id, value.trim());
    renameSession(s.id, value.trim()).catch(() => {});
    setRen(null);
  }

  async function commitDelete() {
    if (!del) return;
    const s = del;
    setDel(null);
    await deleteSession(s.id).catch(() => {});
    setPins((p) => p.filter((x) => x !== s.id));
    setTitles((prev) => {
      const n = { ...prev }; delete n[s.id];
      localStorage.setItem(TITLE_KEY, JSON.stringify(n));
      return n;
    });
    if (sessionRef.current === s.id) newChat();
    refreshSessions();
  }

  async function changeFolder() {
    const dir = await pickWorkspace();
    if (dir) getAgentInfo().then(setAgent);
  }

  async function toggleRuntime(updates: Record<string, string>) {
    if (restarting) return;
    setRestarting(true);
    try {
      await applyConfig(updates);
      await getAgentInfo().then(setAgent); // badge reflects config immediately
    } finally {
      setTimeout(() => setRestarting(false), 9000); // backend cold-start window
    }
  }

  async function changeModel(id: string) {
    if (restarting || !id || id === sub?.model) return;
    setRestarting(true);
    try {
      await applyConfig({ "model.default": id });
      await getSubscription().then(setSub);
    } finally {
      setTimeout(() => setRestarting(false), 9000);
    }
  }

  async function toggleSandbox() {
    if (restarting) return;
    setRestarting(true);
    try {
      await setSandbox(!agent?.sandboxed);
      await getAgentInfo().then(setAgent);
    } finally {
      setTimeout(() => setRestarting(false), 9000);
    }
  }

  useEffect(() => {
    getHealth().then(setHealth);
    getAgentInfo().then(setAgent);
    getSubscription().then(setSub);
    getUsage().then(setUsage);
    getSkills().then(setSkills);
    getWorkspaceDiff().then(setDiffs);
    refreshSessions();
    const id = setInterval(() => getHealth().then(setHealth), 5000);
    return () => clearInterval(id);
  }, []);

  const refreshDiff = () => getWorkspaceDiff().then(setDiffs);

  // in-chat command menu: active when input is exactly "/word" (no space yet)
  const slashMatch = /^\/([\p{L}\w-]*)$/u.exec(input);
  const cmdItems = slashMatch
    ? [
        ...LOCAL_CMDS.map((c) => ({ name: c.name, icon: c.icon, hint: c.hint })),
        ...skills.map((s) => ({ name: s.name, icon: "puzzle" as const, hint: s.description || s.label || "скилл Hermes" })),
      ]
        .filter((c) => c.name.toLowerCase().startsWith(slashMatch[1].toLowerCase()))
        .slice(0, 8)
    : [];

  function pickCmd(name: string) {
    setInput(`/${name} `);
    inputRef.current?.focus();
  }

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    // only stick to bottom if the user hasn't scrolled up (lets them read during streaming)
    if (stickRef.current) el.scrollTop = el.scrollHeight;
  }, [messages]);

  function onChatScroll(e: React.UIEvent<HTMLDivElement>) {
    const el = e.currentTarget;
    stickRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  }

  // First-run gate: decide whether Hermes needs to be installed before the app works.
  useEffect(() => {
    backendStatus().then((st) => setBoot(st === "needs_provision" ? "needs" : "ready"));
  }, []);

  async function runProvision() {
    setBoot("provisioning");
    setProvError(null);
    setProvLog([]);
    setProvStage("download");
    await provision((e) => {
      if (e.kind === "stage") setProvStage(e.stage!);
      else if (e.kind === "log" && e.line) {
        // strip ANSI colour codes + stray control chars the installer emits
        const clean = e.line.replace(/\x1b\[[0-9;]*m/g, "").replace(/[\x00-\x1f\x7f]/g, "").trim();
        if (clean) setProvLog((l) => [...l.slice(-300), clean]);
      }
      else if (e.kind === "done") {
        if (e.ok) { setProvStage("start"); setTimeout(() => restartApp(), 700); }
        else { setProvError(e.message || "ошибка установки"); }
      }
    }).catch((err) => setProvError(String(err)));
  }

  function copyMsg(i: number, text: string) {
    copyText(text);
    setCopied(i);
    setTimeout(() => setCopied((c) => (c === i ? null : c)), 1400);
  }

  function speakMsg(i: number, text: string) {
    if (speaking === i) { stopSpeak(); setSpeaking(null); return; }
    stopSpeak();
    speak(text);
    setSpeaking(i);
  }

  function drawWave() {
    const an = analyserRef.current;
    const canvas = waveCanvasRef.current;
    if (!an || !canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const buf = new Uint8Array(an.fftSize);
    an.getByteTimeDomainData(buf);
    const w = canvas.width, h = canvas.height;
    ctx.clearRect(0, 0, w, h);
    const bars = 56, step = Math.floor(buf.length / bars);
    ctx.fillStyle = "#00c259";
    for (let i = 0; i < bars; i++) {
      let sum = 0;
      for (let j = 0; j < step; j++) sum += Math.abs(buf[i * step + j] - 128);
      const amp = sum / step / 128;
      const bh = Math.max(2, Math.min(h, amp * h * 2.4));
      const x = i * (w / bars);
      ctx.fillRect(x + 1, (h - bh) / 2, w / bars - 2, bh);
    }
    rafRef.current = requestAnimationFrame(drawWave);
  }

  async function startRec() {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;
      chunksRef.current = [];
      const mr = new MediaRecorder(stream);
      mr.ondataavailable = (e) => { if (e.data.size) chunksRef.current.push(e.data); };
      mr.onstop = onRecStop;
      mediaRecRef.current = mr;
      mr.start();
      const ctx = new AudioContext();
      const src = ctx.createMediaStreamSource(stream);
      const an = ctx.createAnalyser();
      an.fftSize = 1024;
      src.connect(an);
      audioCtxRef.current = ctx;
      analyserRef.current = an;
      setRecording(true);
      rafRef.current = requestAnimationFrame(drawWave);
    } catch {
      alert("Нет доступа к микрофону. Разрешите его в Системных настройках → Конфиденциальность → Микрофон.");
    }
  }

  function stopRec() {
    mediaRecRef.current?.stop();
  }

  async function onRecStop() {
    if (rafRef.current) cancelAnimationFrame(rafRef.current);
    streamRef.current?.getTracks().forEach((t) => t.stop());
    audioCtxRef.current?.close().catch(() => {});
    setRecording(false);
    const blob = new Blob(chunksRef.current, { type: mediaRecRef.current?.mimeType || "audio/webm" });
    if (!blob.size) return;
    setTranscribing(true);
    try {
      const dataUrl: string = await new Promise((res) => {
        const fr = new FileReader();
        fr.onloadend = () => res(fr.result as string);
        fr.readAsDataURL(blob);
      });
      const text = await transcribe(dataUrl);
      if (text) setInput((v) => (v ? v.trim() + " " : "") + text);
    } catch {
      /* hub error */
    } finally {
      setTranscribing(false);
      inputRef.current?.focus();
    }
  }

  function patchLast(fn: (m: Msg) => Msg) {
    setMessages((prev) => {
      const next = [...prev];
      next[next.length - 1] = fn(next[next.length - 1]);
      return next;
    });
  }

  function newChat() {
    sessionRef.current = null;
    setMessages([]);
    setInput("");
  }

  async function openSession(s: SessionRow) {
    if (busy) return;
    sessionRef.current = s.id;
    const msgs = await getSessionMessages(s.id);
    setMessages(msgs.map((m) => ({ role: m.role, content: m.content })));
  }

  const IMG_RE = /^\/(img|image|картинка)\s+/i;

  async function imageGen(prompt: string, clean: string) {
    setInput("");
    setBusy(true);
    setMessages((prev) => [
      ...prev,
      { role: "user", content: prompt },
      { role: "assistant", content: `Генерирую изображение: «${clean}»…`, pending: true },
    ]);
    try {
      const url = await generateImage(clean);
      patchLast((m) => ({ ...m, content: "", image: url, pending: false }));
    } catch (e: any) {
      patchLast((m) => ({ ...m, content: `Ошибка: ${e?.message ?? e}`, pending: false }));
    } finally {
      setBusy(false);
    }
  }

  async function send(text: string) {
    const prompt = text.trim();
    if (!prompt || busy) return;
    // app-side companion commands (bare, no args)
    if (prompt === "/new" || prompt === "/clear") { newChat(); return; }
    if (prompt === "/diff") { setInput(""); refreshDiff(); return; }
    if (prompt === "/folder") { setInput(""); changeFolder(); return; }
    if (IMG_RE.test(prompt)) return imageGen(prompt, prompt.replace(IMG_RE, "").trim());
    stickRef.current = true;
    setInput("");
    setBusy(true);
    setMessages((prev) => [
      ...prev,
      { role: "user", content: prompt },
      { role: "assistant", content: "", reasoning: "", tools: [], pending: true },
    ]);

    const wasNew = !sessionRef.current;
    try {
      if (!sessionRef.current) sessionRef.current = await ensureSession();
      const sid = sessionRef.current!;
      if (wasNew && !titles[sid]) {
        generateTitle(prompt).then((t) => {
          if (t) { saveTitle(sid, t); renameSession(sid, t).catch(() => {}); }
        });
      }
      await streamChat(sessionRef.current!, prompt, {
        onDelta: (s) => patchLast((m) => ({ ...m, content: m.content + s })),
        onReasoning: (s) => patchLast((m) => ({ ...m, reasoning: (m.reasoning ?? "") + s })),
        onTool: (name, status) =>
          patchLast((m) => {
            const tools = [...(m.tools ?? [])];
            const i = tools.findIndex((t) => t.name === name);
            if (i >= 0) tools[i] = { name, status };
            else tools.push({ name, status });
            return { ...m, tools };
          }),
        onDone: (usage) => {
          patchLast((m) => ({ ...m, usage, pending: false }));
          const used = usage?.prompt_tokens ?? usage?.total_tokens ?? 0;
          if (used) setCtxUsed(used);
        },
        onError: (message) => patchLast((m) => ({ ...m, content: m.content || `Ошибка: ${message}`, pending: false })),
      });
    } catch (e: any) {
      patchLast((m) => ({ ...m, content: `Ошибка: ${e?.message ?? e}`, pending: false }));
    } finally {
      patchLast((m) => ({ ...m, pending: false }));
      setBusy(false);
      refreshSessions();
      refreshDiff();
      getUsage().then(setUsage);
    }
  }

  const online = health?.status === "ok";
  const workspace = agent?.workspace?.replace(agent?.home ?? "###", "~") ?? null;
  const hasChat = messages.length > 0;

  const composer = (big: boolean) => (
    <div className={`composer ${big ? "big" : ""}`}>
      {cmdItems.length > 0 && (
        <div className="cmd-menu">
          <div className="cmd-head">Команды</div>
          {cmdItems.map((c) => (
            <button
              key={c.name}
              className="cmd-item"
              onMouseDown={(e) => { e.preventDefault(); pickCmd(c.name); }}
            >
              <Icon name={c.icon} size={15} />
              <span className="cmd-name">/{c.name}</span>
              <span className="cmd-hint">{c.hint}</span>
            </button>
          ))}
        </div>
      )}
      <textarea
        ref={inputRef}
        value={input}
        placeholder={online ? "Спроси Hermes…  «/» — команды и скиллы" : "Ожидание бэкенда…"}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey && cmdItems.length === 0) { e.preventDefault(); send(input); }
          else if (e.key === "Enter" && !e.shiftKey && cmdItems.length > 0) { e.preventDefault(); pickCmd(cmdItems[0].name); }
          else if (e.key === "Escape") { (e.target as HTMLTextAreaElement).blur(); }
        }}
        rows={big ? 2 : 1}
      />
      {recording && (
        <div className="wave-wrap">
          <span className="wave-dot" />
          <canvas ref={waveCanvasRef} className="wave" width={640} height={40} />
          <button className="wave-stop" onClick={stopRec} title="Остановить запись">
            <Icon name="stop" size={14} /> готово
          </button>
        </div>
      )}
      <div className="composer-row">
        <button className="composer-hint" onClick={changeFolder} disabled={!isTauri} title="Сменить рабочую папку агента">
          <Icon name="folder" size={15} /> {workspace ?? (isTauri ? "…" : "n/a")}
        </button>
        <button
          className={`mic ${recording ? "rec" : ""}`}
          onClick={recording ? stopRec : startRec}
          disabled={!isTauri || transcribing}
          title={recording ? "Остановить" : "Голосовой ввод"}
        >
          {transcribing ? <span className="spinner" /> : <Icon name="mic" size={17} />}
        </button>
        <button className="send" onClick={() => send(input)} disabled={!online || !input.trim() || busy}>
          {busy ? <span className="spinner" /> : <Icon name="send" size={18} />}
        </button>
      </div>
    </div>
  );

  if (boot === "needs" || boot === "provisioning") {
    const STAGES: [string, string][] = [
      ["download", "Загрузка установщика"],
      ["install", "Python · Node · Hermes · зависимости"],
      ["configure", "Настройка под NeuralDeep hub"],
      ["start", "Запуск"],
    ];
    const curIdx = STAGES.findIndex(([k]) => k === provStage);
    return (
      <div className="setup">
        <div className="setup-card">
          <div className="logo setup-logo">ND</div>
          <h1>Первая настройка</h1>
          <p className="setup-sub">
            Neural Deep установит изолированное окружение Hermes в собственную папку
            (<code>~/.neuraldeep</code>) — твой <code>~/.hermes</code> не затрагивается.
            Загрузка ~1.5 ГБ при первом запуске.
          </p>

          {boot === "needs" && !provError && (
            <button className="setup-go" onClick={runProvision}>
              <Icon name="box" size={16} /> Установить окружение
            </button>
          )}

          {boot === "provisioning" && (
            <div className="setup-stages">
              {STAGES.map(([k, label], i) => (
                <div key={k} className={`setup-stage ${i < curIdx ? "done" : i === curIdx ? "active" : "pending"}`}>
                  <span className="setup-stage-ico">
                    {i < curIdx ? <Icon name="check" size={14} /> : i === curIdx ? <span className="spinner" /> : <Icon name="dot" size={8} />}
                  </span>
                  {label}
                </div>
              ))}
            </div>
          )}

          {provError ? (
            <div className="setup-error">
              <Icon name="alert" size={14} /> {provError}
              <button className="setup-retry" onClick={runProvision}>Повторить</button>
            </div>
          ) : provLog.length > 0 ? (
            <pre className="setup-log">{provLog.slice(-9).join("\n")}</pre>
          ) : null}
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      {/* Sidebar */}
      <aside className="sidebar">
        <div className="side-brand">
          <span className="logo">ND</span>
          <span className="wordmark">neuraldeep</span>
        </div>
        <button className="new-chat" onClick={newChat}><Icon name="plus" size={16} /> Новый чат</button>
        <div className="side-section">Недавние</div>
        <div className="recent-search">
          <Icon name="search" size={13} />
          <input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Поиск чатов…" />
        </div>
        <div className="recents">
          {(() => {
            const q = search.trim().toLowerCase();
            const list = [...sessions]
              .filter((s) => !q || titleOf(s).toLowerCase().includes(q) || s.id.toLowerCase().includes(q))
              .sort((a, b) => (pins.includes(b.id) ? 1 : 0) - (pins.includes(a.id) ? 1 : 0));
            if (list.length === 0) return <div className="recents-empty">{q ? "ничего не найдено" : "пусто"}</div>;
            return list.map((s) => {
              const pinned = pins.includes(s.id);
              return (
                <div
                  key={s.id}
                  className={`recent ${sessionRef.current === s.id ? "active" : ""} ${pinned ? "pinned" : ""}`}
                  onClick={() => openSession(s)}
                  title={`${s.source} · ${s.message_count} msgs`}
                >
                  <span className="recent-dot" data-src={s.source} />
                  <span className="recent-title">{titleOf(s)}</span>
                  <button className="recent-act" onClick={(e) => renameChat(s, e)} title="Переименовать">
                    <Icon name="pencil" size={13} />
                  </button>
                  <button className="pin-btn" onClick={(e) => togglePin(s.id, e)} title={pinned ? "Открепить" : "Закрепить"}>
                    <Icon name={pinned ? "pin" : "pinOff"} size={14} fill={pinned} />
                  </button>
                  <button className="recent-act danger" onClick={(e) => delChat(s, e)} title="Удалить">
                    <Icon name="trash" size={13} />
                  </button>
                </div>
              );
            });
          })()}
        </div>
        <div className="account">
          <div className="account-plan" title={sub?.email ?? ""}>
            <span className="account-k">{cap(sub?.tier)}</span>
            <span className="account-v">{sub ? `${sub.user ?? ""} · ${sub.models ?? "?"} models` : ""}</span>
          </div>
          <label className="reasoning-toggle">
            <input type="checkbox" checked={showReasoning} onChange={(e) => setShowReasoning(e.target.checked)} />
            reasoning
          </label>
        </div>
      </aside>

      {/* Main */}
      <main className="main">
        <header className="topbar">
          <div className="topbar-left">
            <Icon name="chevron" size={13} className="model-caret" />
            <select
              className="model-select"
              value={sub?.model ?? ""}
              onChange={(e) => changeModel(e.target.value)}
              disabled={restarting || !sub?.model_list?.length}
              title="Модель (рестарт backend ~9с)"
            >
              {!sub?.model && <option value="">модель…</option>}
              {sub?.model_list?.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.id}{m.ctx ? ` · ${Math.round(m.ctx / 1000)}k` : ""}
                </option>
              ))}
            </select>
          </div>
          <div className={`engine ${online ? "ok" : "down"}`}>
            <span className="dot" />
            Hermes {health?.version ? `v${health.version}` : ""} {online ? "online" : "offline"} → hub
          </div>
        </header>

        {!hasChat ? (
          <div className="hero">
            <div className="hero-spark"><Icon name="spark" size={30} /></div>
            <h1>Чем займёмся сегодня?</h1>
            <div className="hero-composer">{composer(true)}</div>
            <div className="suggestions">
              {SUGGESTIONS.map((s, i) => (
                <button key={i} className="suggestion" onClick={() => send(s.prompt)} disabled={!online}>
                  <span className="sug-icon"><Icon name={s.icon} size={18} /></span>
                  <span className="sug-label">{s.label}</span>
                </button>
              ))}
            </div>
          </div>
        ) : (
          <>
            <div className="chat" ref={scrollRef} onScroll={onChatScroll}>
              {messages.map((m, i) => (
                <div key={i} className={`row ${m.role}`}>
                  <div className="avatar">{m.role === "user" ? "you" : "nd"}</div>
                  <div className="bubble-wrap">
                    {m.role === "assistant" && m.tools && m.tools.length > 0 && (
                      <div className="tools">
                        {m.tools.map((t, j) => (
                          <span key={j} className={`tool ${t.status}`}><Icon name="tool" size={13} /> {t.name} · {t.status}</span>
                        ))}
                      </div>
                    )}
                    {m.role === "assistant" && m.reasoning && showReasoning && (
                      <details className="reasoning" open>
                        <summary><Icon name="brain" size={14} /> reasoning</summary>
                        <div className="reasoning-body">{m.reasoning}</div>
                      </details>
                    )}
                    <div className="bubble">
                      {m.image ? (
                        <div className="gen-img-wrap">
                          <img className="gen-img" src={m.image} alt="generated" onClick={() => openImage(m.image!)} title="Открыть" />
                          <div className="gen-img-actions">
                            <button onClick={() => openImage(m.image!)}><Icon name="image" size={13} /> Открыть</button>
                            <button onClick={() => saveImage(m.image!)}><Icon name="send" size={13} className="rot180" /> Скачать</button>
                          </div>
                        </div>
                      ) : m.role === "assistant"
                        ? (m.pending
                            ? (m.content ? <>{m.content}<span className="cursor">▋</span></> : <span className="cursor">▋</span>)
                            : (m.content ? <Md>{m.content}</Md> : ""))
                        : m.content}
                    </div>
                    {!m.pending && m.content && (
                      <div className="msg-actions">
                        <button className="msg-act" onClick={() => copyMsg(i, m.content)} title="Скопировать">
                          <Icon name={copied === i ? "check" : "copy"} size={13} />
                          {copied === i ? "скопировано" : "копировать"}
                        </button>
                        {m.role === "assistant" && (
                          <button className={`msg-act ${speaking === i ? "on" : ""}`} onClick={() => speakMsg(i, m.content)} disabled={!isTauri} title="Озвучить">
                            <Icon name={speaking === i ? "stop" : "volume"} size={13} />
                            {speaking === i ? "стоп" : "озвучить"}
                          </button>
                        )}
                        {m.role === "assistant" && m.usage?.total_tokens != null && (
                          <span className="usage">· {m.usage.total_tokens} tokens</span>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
            <div className="composer-dock">{composer(false)}</div>
          </>
        )}

        {/* Bottom status bar */}
        <footer className="statusbar">
          <span className={`sb-item engine-sb ${online ? "ok" : "down"}`}>
            <span className="dot" /> Hermes {health?.version ? `v${health.version}` : ""} {online ? "online" : "offline"} → hub
          </span>
          <span className="sb-sep" />
          <span className="sb-item" title={`${ctxUsed} / ${sub?.ctx ?? "?"} токенов · ${sub?.model ?? ""}`}>
            <Icon name="brain" size={13} />
            <span className="sb-ctx-bar"><span style={{ width: `${Math.min(100, sub?.ctx ? (ctxUsed / sub.ctx) * 100 : 0)}%` }} /></span>
            {fmtK(ctxUsed)}/{fmtK(sub?.ctx)} ctx
          </span>
          <span className="sb-sep" />
          <button
            className={`sb-item sb-btn ${agent?.auto_accept ? "warn" : "good"}`}
            onClick={() => toggleRuntime({ hooks_auto_accept: agent?.auto_accept ? "false" : "true" })}
            disabled={restarting}
            title="Клик: авто-выполнение ↔ по запросу (рестарт backend)"
          >
            <Icon name="shield" size={13} /> {agent?.auto_accept ? "авто-выполнение" : "по запросу"}
          </button>
          <button
            className={`sb-item sb-btn ${agent?.sandboxed ? "good" : ""}`}
            onClick={toggleSandbox}
            disabled={restarting}
            title="Клик: host ↔ Seatbelt sandbox (рестарт backend)"
          >
            <Icon name="box" size={13} /> {restarting ? "рестарт…" : agent?.sandboxed ? "sandbox" : "host"}
          </button>
          <span className="sb-spacer" />
          {usage?.gate ? (
            <span
              className="sb-item"
              title={`Живые лимиты хаба${usage.gate.session ? ` · сессия ${usage.gate.session.used}/${usage.gate.session.limit} (${usage.gate.session.window})` : ""}${usage.gate.week ? ` · неделя ${usage.gate.week.used}/${usage.gate.week.limit}` : ""}`}
            >
              <Icon name="gauge" size={13} />
              {usage.gate.session != null && ` 3ч ${Math.round(usage.gate.session.pct)}%`}
              {usage.gate.week != null && ` · нед ${Math.round(usage.gate.week.pct)}%`}
              {usage.wallet?.balance_rub != null && ` · ₽${usage.wallet.balance_rub}`}
            </span>
          ) : (
            <span className="sb-item muted" title="Лимиты хаба (/api/cli/usage)">
              <Icon name="gauge" size={13} /> лимиты · —
            </span>
          )}
        </footer>
      </main>

      {/* Right: workspace diff (when the agent changed files) */}
      {diffs.length > 0 && (
        <aside className={`diff-panel ${diffOpen ? "" : "collapsed"}`}>
          <div className="diff-head">
            <button className="diff-toggle" onClick={() => setDiffOpen((v) => !v)} title={diffOpen ? "Свернуть" : "Развернуть"}>
              <Icon name="chevron" size={14} className={diffOpen ? "rot" : ""} />
            </button>
            <span className="diff-title">Изменения</span>
            <span className="diff-count">{diffs.length}</span>
            <button className="diff-refresh" onClick={refreshDiff} title="Обновить дифф"><Icon name="search" size={14} /></button>
          </div>
          {diffOpen && (
            <div className="diff-body">
              {diffs.map((d) => (
                <details key={d.path} className="diff-file" open={diffs.length <= 3}>
                  <summary>
                    <span className={`diff-status s-${d.status[0]}`}>{d.status[0]}</span>
                    <span className="diff-path">{d.path}</span>
                  </summary>
                  <Patch text={d.patch} />
                </details>
              ))}
            </div>
          )}
        </aside>
      )}

      {/* Modal: rename / delete confirm (WebView blocks native prompt/confirm) */}
      {(del || ren) && (
        <div className="modal-overlay" onClick={() => { setDel(null); setRen(null); }}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            {ren ? (
              <>
                <div className="modal-title">Переименовать чат</div>
                <input
                  className="modal-input"
                  autoFocus
                  value={ren.value}
                  onChange={(e) => setRen({ ...ren, value: e.target.value })}
                  onKeyDown={(e) => { if (e.key === "Enter") commitRename(); if (e.key === "Escape") setRen(null); }}
                />
                <div className="modal-actions">
                  <button className="modal-btn" onClick={() => setRen(null)}>Отмена</button>
                  <button className="modal-btn primary" onClick={commitRename}>Сохранить</button>
                </div>
              </>
            ) : (
              <>
                <div className="modal-title">Удалить чат?</div>
                <div className="modal-text">«{del && titleOf(del)}» — действие необратимо.</div>
                <div className="modal-actions">
                  <button className="modal-btn" onClick={() => setDel(null)}>Отмена</button>
                  <button className="modal-btn danger" onClick={commitDelete}>Удалить</button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
