import { useEffect, useRef, useState } from "react";
import {
  getHealth, getAgentInfo, getSubscription, getSessions, getSessionMessages,
  ensureSession, streamChat, pickWorkspace, isTauri,
  type Health, type AgentInfo, type Subscription, type SessionRow,
} from "./transport";
import { Md } from "./Md";

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
}

const cap = (s?: string) => (s ? s.charAt(0).toUpperCase() + s.slice(1) : "—");

const SUGGESTIONS = [
  { icon: "✅", label: "Проверить free-тариф", prompt: "Reply with exactly: NEURALDEEP_FREE_TIER_OK" },
  { icon: "💭", label: "Показать reasoning (17×23)", prompt: "Сколько будет 17*23? Думай пошагово, потом ответь." },
  { icon: "🧩", label: "Что ты умеешь?", prompt: "Кратко: кто ты, какая модель отвечает и какие у тебя возможности?" },
];

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
  const sessionRef = useRef<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

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
    refreshSessions();
    const id = setInterval(() => getHealth().then(setHealth), 5000);
    return () => clearInterval(id);
  }, []);

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

  async function send(text: string) {
    const prompt = text.trim();
    if (!prompt || busy) return;
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
        onDone: (usage) => patchLast((m) => ({ ...m, usage, pending: false })),
        onError: (message) => patchLast((m) => ({ ...m, content: m.content || `⚠️ ${message}`, pending: false })),
      });
    } catch (e: any) {
      patchLast((m) => ({ ...m, content: `⚠️ ${e?.message ?? e}`, pending: false }));
    } finally {
      patchLast((m) => ({ ...m, pending: false }));
      setBusy(false);
      refreshSessions();
    }
  }

  const online = health?.status === "ok";
  const workspace = agent?.workspace?.replace(agent?.home ?? "###", "~") ?? null;
  const hasChat = messages.length > 0;

  const composer = (big: boolean) => (
    <div className={`composer ${big ? "big" : ""}`}>
      <textarea
        value={input}
        placeholder={online ? "Спроси Hermes…  / для скиллов" : "Ожидание бэкенда…"}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); send(input); }
        }}
        rows={big ? 2 : 1}
      />
      <div className="composer-row">
        <button className="composer-hint" onClick={changeFolder} disabled={!isTauri} title="Сменить рабочую папку агента">
          📁 {workspace ?? (isTauri ? "…" : "n/a")}
        </button>
        <button className="send" onClick={() => send(input)} disabled={!online || !input.trim() || busy}>
          {busy ? "…" : "↑"}
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
        <button className="new-chat" onClick={newChat}>＋ Новый чат</button>
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
                  {pinned ? "📌" : "📍"}
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
            <div className="hero-spark">✳</div>
            <h1>Чем займёмся сегодня?</h1>
            <div className="hero-composer">{composer(true)}</div>
            <div className="suggestions">
              {SUGGESTIONS.map((s, i) => (
                <button key={i} className="suggestion" onClick={() => send(s.prompt)} disabled={!online}>
                  <span className="sug-icon">{s.icon}</span>
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
                          <span key={j} className={`tool ${t.status}`}>🔧 {t.name} · {t.status}</span>
                        ))}
                      </div>
                    )}
                    {m.role === "assistant" && m.reasoning && showReasoning && (
                      <details className="reasoning" open>
                        <summary>💭 reasoning</summary>
                        <div className="reasoning-body">{m.reasoning}</div>
                      </details>
                    )}
                    <div className="bubble">
                      {m.role === "assistant"
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
      </main>
    </div>
  );
}
