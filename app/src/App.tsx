import { useEffect, useRef, useState } from "react";
import {
  getHealth, getAgentInfo, getSubscription, getSessions, getSessionMessages,
  ensureSession, streamChat, pickWorkspace, generateImage, getWorkspaceDiff, getSkills, isTauri,
  type Health, type AgentInfo, type Subscription, type SessionRow, type DiffFile, type SkillRow,
} from "./transport";
import { Md } from "./Md";
import { Icon } from "./Icon";

const PIN_KEY = "nd.pinnedSessions";
const loadPins = (): string[] => {
  try { return JSON.parse(localStorage.getItem(PIN_KEY) || "[]"); } catch { return []; }
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
] as const;

export function App() {
  const [messages, setMessages] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [health, setHealth] = useState<Health | null>(null);
  const [agent, setAgent] = useState<AgentInfo | null>(null);
  const [sub, setSub] = useState<Subscription | null>(null);
  const [showReasoning, setShowReasoning] = useState(true);
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [pins, setPins] = useState<string[]>(loadPins);
  const [skills, setSkills] = useState<SkillRow[]>([]);
  const [diffs, setDiffs] = useState<DiffFile[]>([]);
  const [diffOpen, setDiffOpen] = useState(true);
  const [ctxUsed, setCtxUsed] = useState(0);
  const sessionRef = useRef<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const refreshSessions = () => getSessions().then((s) => setSessions(s.slice(0, 40)));

  function togglePin(id: string, e: React.MouseEvent) {
    e.stopPropagation();
    setPins((prev) => {
      const next = prev.includes(id) ? prev.filter((x) => x !== id) : [id, ...prev];
      localStorage.setItem(PIN_KEY, JSON.stringify(next));
      return next;
    });
  }

  async function changeFolder() {
    const dir = await pickWorkspace();
    if (dir) getAgentInfo().then(setAgent);
  }

  useEffect(() => {
    getHealth().then(setHealth);
    getAgentInfo().then(setAgent);
    getSubscription().then(setSub);
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
        ...LOCAL_CMDS.map((c) => ({ name: c.name, icon: "image" as const, hint: c.hint })),
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
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" });
  }, [messages]);

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
    if (IMG_RE.test(prompt)) return imageGen(prompt, prompt.replace(IMG_RE, "").trim());
    setInput("");
    setBusy(true);
    setMessages((prev) => [
      ...prev,
      { role: "user", content: prompt },
      { role: "assistant", content: "", reasoning: "", tools: [], pending: true },
    ]);

    try {
      if (!sessionRef.current) sessionRef.current = await ensureSession();
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
      <div className="composer-row">
        <button className="composer-hint" onClick={changeFolder} disabled={!isTauri} title="Сменить рабочую папку агента">
          <Icon name="folder" size={15} /> {workspace ?? (isTauri ? "…" : "n/a")}
        </button>
        <button className="send" onClick={() => send(input)} disabled={!online || !input.trim() || busy}>
          {busy ? <span className="spinner" /> : <Icon name="send" size={18} />}
        </button>
      </div>
    </div>
  );

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
        <div className="recents">
          {sessions.length === 0 && <div className="recents-empty">пусто</div>}
          {[...sessions].sort((a, b) => (pins.includes(b.id) ? 1 : 0) - (pins.includes(a.id) ? 1 : 0)).map((s) => {
            const pinned = pins.includes(s.id);
            return (
              <div
                key={s.id}
                className={`recent ${sessionRef.current === s.id ? "active" : ""} ${pinned ? "pinned" : ""}`}
                onClick={() => openSession(s)}
                title={`${s.source} · ${s.message_count} msgs`}
              >
                <span className="recent-dot" data-src={s.source} />
                <span className="recent-title">{s.title || s.id.slice(0, 22)}</span>
                <button className="pin-btn" onClick={(e) => togglePin(s.id, e)} title={pinned ? "Открепить" : "Закрепить"}>
                  <Icon name={pinned ? "pin" : "pinOff"} size={14} fill={pinned} />
                </button>
              </div>
            );
          })}
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
            <div className="chat" ref={scrollRef}>
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
                        <a href={m.image} download="neuraldeep.png" className="gen-img-link">
                          <img className="gen-img" src={m.image} alt="generated" />
                        </a>
                      ) : m.role === "assistant"
                        ? (m.pending
                            ? (m.content ? <>{m.content}<span className="cursor">▋</span></> : <span className="cursor">▋</span>)
                            : (m.content ? <Md>{m.content}</Md> : ""))
                        : m.content}
                    </div>
                    {m.role === "assistant" && m.usage?.total_tokens && (
                      <div className="usage">{m.usage.total_tokens} tokens</div>
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
          <span className={`sb-item ${agent?.auto_accept ? "warn" : ""}`} title="Режим выполнения инструментов агентом">
            <Icon name="shield" size={13} /> {agent?.auto_accept ? "авто-выполнение" : "по запросу"}
          </span>
          <span className="sb-item" title="Изоляция выполнения инструментов">
            <Icon name="box" size={13} /> {agent?.sandboxed ? "sandbox" : "host"}
          </span>
          <span className="sb-spacer" />
          <span className="sb-item muted" title="Час/неделя — ждёт эндпоинт хаба /api/cli/usage">
            <Icon name="gauge" size={13} /> лимиты · скоро
          </span>
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
    </div>
  );
}
