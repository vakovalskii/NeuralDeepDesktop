// Transport abstraction. Under Tauri the Rust host drives the Hermes session chat
// stream and relays rich events over a Channel. In a browser dev tab we hit the same
// Hermes session API through the Vite /hermes proxy and parse the named SSE events here.

export interface Health {
  status: string;
  version?: string;
}
export interface AgentInfo {
  workspace: string;
  home: string;
  auto_accept?: boolean;
  sandboxed?: boolean;
}
export interface Subscription {
  tier?: string;
  user?: string;
  email?: string;
  rpm?: number;
  parallel?: number;
  models?: number;
  model?: string;
  ctx?: number;
}
export interface RichHandlers {
  onDelta: (s: string) => void;
  onReasoning: (s: string) => void;
  onTool: (name: string, status: string) => void;
  onDone: (usage: any) => void;
  onError: (message: string) => void;
}

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function core() {
  return import("@tauri-apps/api/core");
}

export async function getHealth(): Promise<Health | null> {
  try {
    if (isTauri) return (await (await core()).invoke("hermes_health")) as Health;
    const r = await fetch("/hermes/health");
    return r.ok ? await r.json() : null;
  } catch {
    return null;
  }
}

export async function pickWorkspace(): Promise<string | null> {
  try {
    if (isTauri) return (await (await core()).invoke("pick_workspace")) as string | null;
  } catch {
    /* web: no native dialog */
  }
  return null;
}

/** Generate an image via the hub Image API (FLUX). Returns a data: URL. Tauri-only. */
export async function generateImage(prompt: string, aspect = "1:1"): Promise<string> {
  if (!isTauri) throw new Error("генерация картинок доступна только в десктоп-приложении");
  return (await (await core()).invoke("generate_image", { prompt, aspect })) as string;
}

/** Save a data: image via a native dialog. Returns the saved path or null. Tauri-only. */
export async function saveImage(dataUrl: string): Promise<string | null> {
  if (!isTauri) { const a = document.createElement("a"); a.href = dataUrl; a.download = "neuraldeep.png"; a.click(); return null; }
  return (await (await core()).invoke("save_image", { dataUrl })) as string | null;
}

/** Open a data: image in the OS default viewer. Tauri-only. */
export async function openImage(dataUrl: string): Promise<void> {
  if (!isTauri) { window.open(dataUrl, "_blank"); return; }
  await (await core()).invoke("open_image", { dataUrl });
}

/** Apply config.yaml updates (keys: "field" or "section.field") + restart gateway. Tauri-only. */
export async function applyConfig(updates: Record<string, string>): Promise<void> {
  if (!isTauri) return;
  const c = await core();
  await c.invoke("set_config", { updates });
  await c.invoke("restart_backend");
}

export interface DiffFile { path: string; status: string; patch: string }

/** Unified diff of the agent working dir (git). Empty if not a repo / no changes. Tauri-only. */
export async function getWorkspaceDiff(): Promise<DiffFile[]> {
  try {
    if (isTauri) return (await (await core()).invoke("workspace_diff")) as DiffFile[];
  } catch { /* not a repo / no git */ }
  return [];
}

export interface SkillRow { name: string; label?: string; description?: string }

/** Hermes skills (for the in-chat command menu). */
export async function getSkills(): Promise<SkillRow[]> {
  try {
    const r = await hermesGet("/v1/skills");
    const data = (r?.data ?? r ?? []) as any[];
    return data.map((s) => ({ name: s.name ?? s.id, label: s.label, description: s.description }));
  } catch {
    return [];
  }
}

export async function getAgentInfo(): Promise<AgentInfo | null> {
  try {
    if (isTauri) return (await (await core()).invoke("agent_info")) as AgentInfo;
  } catch {
    /* web: not available */
  }
  return null;
}

export async function getSubscription(): Promise<Subscription | null> {
  try {
    if (isTauri) return (await (await core()).invoke("subscription")) as Subscription;
  } catch {
    /* web: hub not reachable from the browser */
  }
  return null;
}

export interface SessionRow {
  id: string;
  title: string | null;
  source: string;
  message_count: number;
  started_at?: number;
}

async function hermesGet(path: string): Promise<any> {
  if (isTauri) return (await (await core()).invoke("hermes_get", { path })) as any;
  const r = await fetch(`/hermes${path}`);
  return r.ok ? await r.json() : null;
}

export async function getSessions(): Promise<SessionRow[]> {
  try {
    const j = await hermesGet("/api/sessions");
    const arr = j?.data ?? j?.sessions ?? [];
    return arr.map((s: any) => ({
      id: s.id, title: s.title, source: s.source,
      message_count: s.message_count ?? 0, started_at: s.started_at,
    }));
  } catch {
    return [];
  }
}

export async function getSessionMessages(id: string): Promise<{ role: "user" | "assistant"; content: string }[]> {
  try {
    const j = await hermesGet(`/api/sessions/${id}/messages`);
    const arr = j?.messages ?? j?.data ?? [];
    return arr
      .filter((m: any) => (m.role === "user" || m.role === "assistant") && typeof m.content === "string" && m.content.trim())
      .map((m: any) => ({ role: m.role, content: m.content }));
  } catch {
    return [];
  }
}

export async function ensureSession(): Promise<string> {
  if (isTauri) return (await (await core()).invoke("create_session")) as string;
  const r = await fetch("/hermes/api/sessions", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
  const j = await r.json();
  return j?.session?.id ?? j?.session?.session_id;
}

export async function streamChat(sessionId: string, message: string, h: RichHandlers): Promise<void> {
  if (isTauri) return streamViaTauri(sessionId, message, h);
  return streamViaFetch(sessionId, message, h);
}

async function streamViaTauri(sessionId: string, message: string, h: RichHandlers): Promise<void> {
  const { invoke, Channel } = await core();
  const channel = new Channel<any>();
  channel.onmessage = (ev) => {
    switch (ev.kind) {
      case "delta": return h.onDelta(ev.content ?? "");
      case "reasoning": return h.onReasoning(ev.content ?? "");
      case "tool": return h.onTool(ev.name ?? "", ev.status ?? "");
      case "done": return h.onDone(ev.usage);
      case "error": return h.onError(ev.message ?? "error");
    }
  };
  try {
    await invoke("chat_stream", { sessionId, message, onEvent: channel });
  } catch (e: any) {
    h.onError(e?.toString?.() ?? "invoke failed");
  }
}

async function streamViaFetch(sessionId: string, message: string, h: RichHandlers): Promise<void> {
  try {
    const resp = await fetch(`/hermes/api/sessions/${sessionId}/chat/stream`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ message }),
    });
    if (!resp.ok || !resp.body) throw new Error(`Hermes ${resp.status}: ${await resp.text().catch(() => "")}`);
    const reader = resp.body.getReader();
    const decoder = new TextDecoder();
    let buf = "";
    let cur = "";
    let gotContent = false;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      const lines = buf.split("\n");
      buf = lines.pop() ?? "";
      for (const raw of lines) {
        const line = raw.trimEnd();
        if (line.startsWith("event:")) cur = line.slice(6).trim();
        else if (line.startsWith("data:")) {
          let p: any;
          try {
            p = JSON.parse(line.slice(5).trim());
          } catch {
            continue;
          }
          if (cur === "assistant.delta" && p.delta) { gotContent = true; h.onDelta(p.delta); }
          else if (cur === "reasoning.delta" && p.delta) h.onReasoning(p.delta);
          else if (cur === "assistant.completed" && !gotContent && p.content) { gotContent = true; h.onDelta(p.content); }
          else if (cur === "tool.progress") {
            if (p.tool_name === "_thinking") { if (p.delta) h.onReasoning(p.delta); }
            else if (p.tool_name) h.onTool(p.tool_name, "progress");
          } else if (cur === "tool.started" || cur === "tool.completed" || cur === "tool.failed")
            h.onTool(p.tool_name ?? "tool", cur.replace("tool.", ""));
          else if (cur === "run.completed") h.onDone(p.usage);
          else if (cur === "error") h.onError(p.message ?? "error");
        }
      }
    }
  } catch (err: any) {
    h.onError(err?.message ?? String(err));
  }
}
