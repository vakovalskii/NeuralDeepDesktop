// Neural Deep Desktop — Rust host.
//
// Responsibilities (task_06 / task_08):
//   * read the loopback API key (~/.hermes/.env) and the NeuralDeep hub key (config.yaml)
//   * spawn & supervise the Hermes gateway (OpenAI-compatible API on :8642)
//   * be the trusted loopback client: drive the Hermes *session chat stream* and relay
//     rich events (content / reasoning / tool steps) to the webview over a Channel
//   * expose the agent working dir and the hub subscription/tariff status

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Manager, State};

const HERMES_BASE: &str = "http://127.0.0.1:8642";
const HUB_STATUS: &str = "https://hub.neuraldeep.ru/api/cli/status";

#[derive(Default)]
struct Backend {
    child: Mutex<Option<Child>>,
}

/// Live, user-changeable agent working directory.
struct WorkspaceState(Mutex<String>);

#[derive(Clone)]
struct Config {
    api_key: String,    // Hermes API_SERVER_KEY (loopback)
    hub_key: String,    // NeuralDeep hub key
    workspace: String,  // dir the agent runs in
    home: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum StreamEvent {
    Delta { content: String },
    Reasoning { content: String },
    Tool { name: String, status: String },
    Done { usage: serde_json::Value },
    Error { message: String },
}

fn home() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_default()
}
fn hermes_home() -> std::path::PathBuf {
    home().join(".hermes")
}

/// Read a `KEY=value` line from ~/.hermes/.env.
fn read_env_key(name: &str) -> String {
    if let Ok(v) = std::env::var(name) {
        if !v.is_empty() {
            return v;
        }
    }
    if let Ok(txt) = std::fs::read_to_string(hermes_home().join(".env")) {
        for line in txt.lines() {
            if let Some(rest) = line.trim().strip_prefix(&format!("{name}=")) {
                return rest.trim().to_string();
            }
        }
    }
    String::new()
}

/// Pull the hub key from config.yaml (`api_key:`) as a fallback.
fn read_hub_key() -> String {
    let k = read_env_key("NEURALDEEP_API_KEY");
    if !k.is_empty() {
        return k;
    }
    if let Ok(txt) = std::fs::read_to_string(hermes_home().join("config.yaml")) {
        for line in txt.lines() {
            if let Some(rest) = line.trim().strip_prefix("api_key:") {
                return rest.trim().to_string();
            }
        }
    }
    String::new()
}

async fn fetch_health() -> Option<serde_json::Value> {
    reqwest::Client::new()
        .get(format!("{HERMES_BASE}/health"))
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()
}

fn hermes_launcher() -> Option<String> {
    let candidate = home().join(".local/bin/hermes");
    if candidate.exists() {
        return Some(candidate.to_string_lossy().into_owned());
    }
    Some("hermes".to_string())
}

fn ensure_backend(backend: &Backend, cfg: &Config) {
    if tauri::async_runtime::block_on(fetch_health()).is_some() {
        eprintln!("[nd] reusing already-running Hermes backend on {HERMES_BASE}");
        return;
    }
    let Some(launcher) = hermes_launcher() else {
        eprintln!("[nd] could not locate the `hermes` launcher");
        return;
    };
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(hermes_home().join("logs/devrun-gateway.log"));
    let (out, err) = match log {
        Ok(f) => (Stdio::from(f.try_clone().unwrap()), Stdio::from(f)),
        Err(_) => (Stdio::null(), Stdio::null()),
    };
    eprintln!("[nd] starting Hermes gateway via {launcher} (cwd={})", cfg.workspace);
    match Command::new(&launcher)
        .arg("gateway")
        .current_dir(&cfg.workspace)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .spawn()
    {
        Ok(child) => *backend.child.lock().unwrap() = Some(child),
        Err(e) => eprintln!("[nd] failed to spawn gateway: {e}"),
    }
}

#[tauri::command]
async fn hermes_health() -> serde_json::Value {
    fetch_health().await.unwrap_or(serde_json::json!({ "status": "down" }))
}

/// Generic authenticated GET against the Hermes loopback API (e.g. /api/sessions).
#[tauri::command]
async fn hermes_get(path: String, config: State<'_, Config>) -> Result<serde_json::Value, String> {
    let resp = reqwest::Client::new()
        .get(format!("{HERMES_BASE}{path}"))
        .header("Authorization", format!("Bearer {}", config.api_key))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    resp.json::<serde_json::Value>().await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct AgentInfo {
    workspace: String,
    home: String,
}

#[tauri::command]
fn agent_info(ws: State<'_, WorkspaceState>, config: State<'_, Config>) -> AgentInfo {
    AgentInfo {
        workspace: ws.0.lock().unwrap().clone(),
        home: config.home.clone(),
    }
}

/// Persist the agent terminal cwd into ~/.hermes/config.yaml (best-effort).
fn persist_workspace_to_config(dir: &str) {
    let path = hermes_home().join("config.yaml");
    let Ok(txt) = std::fs::read_to_string(&path) else { return };
    let mut out = String::new();
    let mut in_terminal = false;
    let mut replaced = false;
    for line in txt.lines() {
        let trimmed = line.trim_start();
        if !line.starts_with(' ') && line.contains(':') {
            in_terminal = trimmed.starts_with("terminal:");
        }
        if in_terminal && trimmed.starts_with("cwd:") {
            let indent = &line[..line.len() - line.trim_start().len()];
            out.push_str(&format!("{indent}cwd: {dir}\n"));
            replaced = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if replaced {
        let _ = std::fs::write(&path, out);
    }
}

/// Open a native folder picker and set the agent working directory.
#[tauri::command]
async fn pick_workspace(ws: State<'_, WorkspaceState>) -> Result<Option<String>, String> {
    let picked = rfd::AsyncFileDialog::new().pick_folder().await;
    match picked {
        Some(handle) => {
            let dir = handle.path().to_string_lossy().into_owned();
            *ws.0.lock().unwrap() = dir.clone();
            persist_workspace_to_config(&dir);
            Ok(Some(dir))
        }
        None => Ok(None),
    }
}

/// Hub subscription / tariff (NeuralDeep hub `/api/cli/status`).
#[tauri::command]
async fn subscription(config: State<'_, Config>) -> Result<serde_json::Value, String> {
    let key = config.hub_key.clone();
    if key.is_empty() {
        return Err("no hub key".into());
    }
    let resp = reqwest::Client::new()
        .get(HUB_STATUS)
        .header("Authorization", format!("Bearer {key}"))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "tier": json["tier"],
        "user": json["user"]["name"],
        "email": json["user"]["email"],
        "rpm": json["limits"]["rpm"],
        "parallel": json["limits"]["parallel"],
        "models": json["models"].as_array().map(|a| a.len()).unwrap_or(0),
    }))
}

#[tauri::command]
async fn create_session(config: State<'_, Config>) -> Result<String, String> {
    let resp = reqwest::Client::new()
        .post(format!("{HERMES_BASE}/api/sessions"))
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    json["session"]["id"]
        .as_str()
        .or_else(|| json["session"]["session_id"].as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "no session id in response".into())
}

/// Drive the Hermes session chat stream and relay named SSE events.
#[tauri::command]
async fn chat_stream(
    session_id: String,
    message: String,
    on_event: Channel<StreamEvent>,
    config: State<'_, Config>,
) -> Result<(), String> {
    let url = format!("{HERMES_BASE}/api/sessions/{session_id}/chat/stream");
    let resp = match reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "message": message }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = on_event.send(StreamEvent::Error { message: e.to_string() });
            return Ok(());
        }
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let _ = on_event.send(StreamEvent::Error { message: format!("Hermes {status}: {text}") });
        return Ok(());
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut cur_event = String::new();
    let mut got_content = false;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let _ = on_event.send(StreamEvent::Error { message: e.to_string() });
                return Ok(());
            }
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        loop {
            let Some(nl) = buf.find('\n') else { break };
            let line = buf[..nl].to_string();
            buf.drain(..=nl);
            let line = line.trim_end();
            if let Some(name) = line.strip_prefix("event:") {
                cur_event = name.trim().to_string();
            } else if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                let Ok(p) = serde_json::from_str::<serde_json::Value>(data) else { continue };
                match cur_event.as_str() {
                    "assistant.delta" => {
                        if let Some(d) = p["delta"].as_str() {
                            if !d.is_empty() {
                                got_content = true;
                                let _ = on_event.send(StreamEvent::Delta { content: d.to_string() });
                            }
                        }
                    }
                    "assistant.completed" => {
                        // Fallback: if the answer never streamed as deltas (reasoning models
                        // sometimes deliver it only at completion), emit it now.
                        if !got_content {
                            if let Some(c) = p["content"].as_str() {
                                if !c.is_empty() {
                                    got_content = true;
                                    let _ = on_event.send(StreamEvent::Delta { content: c.to_string() });
                                }
                            }
                        }
                    }
                    "reasoning.delta" => {
                        if let Some(d) = p["delta"].as_str() {
                            if !d.is_empty() {
                                let _ = on_event.send(StreamEvent::Reasoning { content: d.to_string() });
                            }
                        }
                    }
                    "tool.progress" => {
                        let name = p["tool_name"].as_str().unwrap_or("");
                        let d = p["delta"].as_str().unwrap_or("");
                        if name == "_thinking" {
                            if !d.is_empty() {
                                let _ = on_event.send(StreamEvent::Reasoning { content: d.to_string() });
                            }
                        } else if !name.is_empty() {
                            let _ = on_event.send(StreamEvent::Tool { name: name.to_string(), status: "progress".into() });
                        }
                    }
                    "tool.started" | "tool.completed" | "tool.failed" => {
                        let name = p["tool_name"].as_str().unwrap_or("tool").to_string();
                        let status = cur_event.trim_start_matches("tool.").to_string();
                        let _ = on_event.send(StreamEvent::Tool { name, status });
                    }
                    "run.completed" => {
                        let _ = on_event.send(StreamEvent::Done { usage: p["usage"].clone() });
                    }
                    "error" => {
                        let _ = on_event.send(StreamEvent::Error {
                            message: p["message"].as_str().unwrap_or("error").to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let home_s = home().to_string_lossy().into_owned();
    let cfg = Config {
        api_key: read_env_key("API_SERVER_KEY"),
        hub_key: read_hub_key(),
        workspace: home_s.clone(),
        home: home_s,
    };

    tauri::Builder::default()
        .manage(cfg.clone())
        .manage(Backend::default())
        .manage(WorkspaceState(Mutex::new(cfg.workspace.clone())))
        .invoke_handler(tauri::generate_handler![
            hermes_health,
            hermes_get,
            agent_info,
            subscription,
            pick_workspace,
            create_session,
            chat_stream
        ])
        .setup(move |app| {
            let backend = app.state::<Backend>();
            ensure_backend(&backend, &cfg);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(backend) = window.app_handle().try_state::<Backend>() {
                    if let Some(mut child) = backend.child.lock().unwrap().take() {
                        let _ = child.kill();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Neural Deep Desktop");
}
