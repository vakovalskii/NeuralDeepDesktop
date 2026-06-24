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
const HUB_IMG: &str = "https://api.neuraldeep.ru/v1/images";

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

/// Marker file: presence with "1" means the gateway runs under the Seatbelt sandbox.
fn sandbox_marker() -> std::path::PathBuf {
    hermes_home().join(".nd-sandbox")
}
fn sandbox_enabled() -> bool {
    std::fs::read_to_string(sandbox_marker())
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

/// Generate a macOS Seatbelt (sandbox-exec) profile: full read/network/exec,
/// but file-writes confined to the workspace, ~/.hermes, caches and temp.
fn write_seatbelt_profile(workspace: &str) -> std::path::PathBuf {
    let hh = hermes_home();
    let home = home();
    let profile = format!(
        "(version 1)\n\
         (allow default)\n\
         (deny file-write*)\n\
         (allow file-write*\n\
         \x20 (subpath \"{ws}\")\n\
         \x20 (subpath \"{hh}\")\n\
         \x20 (subpath \"/tmp\")\n\
         \x20 (subpath \"/private/tmp\")\n\
         \x20 (subpath \"/private/var/folders\")\n\
         \x20 (subpath \"{home}/Library/Caches\")\n\
         \x20 (subpath \"{home}/.cache\")\n\
         \x20 (subpath \"/dev\"))\n",
        ws = workspace,
        hh = hh.to_string_lossy(),
        home = home.to_string_lossy(),
    );
    let path = hh.join(".nd-sandbox.sb");
    let _ = std::fs::write(&path, profile);
    path
}

/// Toggle the Seatbelt sandbox marker (takes effect on next gateway start).
#[tauri::command]
fn set_sandbox(on: bool) -> Result<(), String> {
    std::fs::write(sandbox_marker(), if on { "1" } else { "0" }).map_err(|e| e.to_string())
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
    let sandboxed = sandbox_enabled();
    eprintln!(
        "[nd] starting Hermes gateway via {launcher} (cwd={}, sandbox={sandboxed})",
        cfg.workspace
    );
    let mut cmd = if sandboxed {
        let profile = write_seatbelt_profile(&cfg.workspace);
        let mut c = Command::new("sandbox-exec");
        c.arg("-f").arg(&profile).arg(&launcher).arg("gateway");
        c
    } else {
        let mut c = Command::new(&launcher);
        c.arg("gateway");
        c
    };
    match cmd
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

/// Delete a Hermes session.
#[tauri::command]
async fn delete_session(session_id: String, config: State<'_, Config>) -> Result<(), String> {
    reqwest::Client::new()
        .delete(format!("{HERMES_BASE}/api/sessions/{session_id}"))
        .header("Authorization", format!("Bearer {}", config.api_key))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Rename a Hermes session (PATCH title).
#[tauri::command]
async fn rename_session(
    session_id: String,
    title: String,
    config: State<'_, Config>,
) -> Result<(), String> {
    reqwest::Client::new()
        .patch(format!("{HERMES_BASE}/api/sessions/{session_id}"))
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&serde_json::json!({ "title": title }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Generate a short chat title from text via a fast hub model.
#[tauri::command]
async fn generate_title(text: String, config: State<'_, Config>) -> Result<String, String> {
    let key = config.hub_key.clone();
    if key.is_empty() {
        return Err("no hub key".into());
    }
    let body = serde_json::json!({
        "model": "gemma-4-31b-noreason",
        "max_tokens": 24,
        "messages": [
            {"role": "system", "content": "Придумай очень короткий заголовок (3-5 слов) для диалога по первому сообщению. Ответь только заголовком, без кавычек и точки."},
            {"role": "user", "content": text}
        ]
    });
    let j: serde_json::Value = reqwest::Client::new()
        .post("https://api.neuraldeep.ru/v1/chat/completions")
        .header("Authorization", format!("Bearer {key}"))
        .json(&body)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let title = j["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .to_string();
    if title.is_empty() {
        Err("empty title".into())
    } else {
        Ok(title)
    }
}

#[derive(Serialize)]
struct AgentInfo {
    workspace: String,
    home: String,
    auto_accept: bool,
    sandboxed: bool,
}

#[tauri::command]
fn agent_info(ws: State<'_, WorkspaceState>, config: State<'_, Config>) -> AgentInfo {
    let cfg = std::fs::read_to_string(hermes_home().join("config.yaml")).unwrap_or_default();
    let auto_accept = cfg
        .lines()
        .any(|l| l.trim_start().starts_with("hooks_auto_accept:") && l.contains("true"));
    let sandboxed = sandbox_enabled();
    AgentInfo {
        workspace: ws.0.lock().unwrap().clone(),
        home: config.home.clone(),
        sandboxed,
        auto_accept,
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

#[derive(Serialize)]
struct DiffFile {
    path: String,
    status: String,
    patch: String,
}

/// Unified diff of the agent's working dir (tracked changes + untracked files).
/// Empty when the dir isn't a git repo or there's nothing to show.
#[tauri::command]
fn workspace_diff(ws: State<'_, WorkspaceState>) -> Result<Vec<DiffFile>, String> {
    let dir = ws.0.lock().unwrap().clone();
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    // git repo?
    if git(&["rev-parse", "--is-inside-work-tree"]).map(|s| s.trim().to_string())
        != Some("true".into())
    {
        return Ok(vec![]);
    }
    let mut files = Vec::new();
    // tracked changes
    if let Some(names) = git(&["diff", "--name-status"]) {
        for line in names.lines() {
            let mut it = line.split_whitespace();
            let status = it.next().unwrap_or("M").to_string();
            let Some(path) = it.next() else { continue };
            let patch = git(&["diff", "--no-color", "--", path]).unwrap_or_default();
            files.push(DiffFile { path: path.into(), status, patch });
        }
    }
    // untracked → render as additions
    if let Some(list) = git(&["ls-files", "--others", "--exclude-standard"]) {
        for path in list.lines().filter(|p| !p.is_empty()) {
            let content =
                std::fs::read_to_string(std::path::Path::new(&dir).join(path)).unwrap_or_default();
            let body: String = content
                .lines()
                .take(500)
                .map(|l| format!("+{l}\n"))
                .collect();
            files.push(DiffFile {
                path: path.into(),
                status: "A".into(),
                patch: format!("@@ new file @@\n{body}"),
            });
        }
    }
    Ok(files)
}

fn decode_data_url(data_url: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let b64 = data_url.split(',').nth(1)?;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

/// Save a `data:` image via a native Save dialog. Returns the path, or None if cancelled.
#[tauri::command]
async fn save_image(data_url: String) -> Result<Option<String>, String> {
    let bytes = decode_data_url(&data_url).ok_or("bad data url")?;
    let file = rfd::AsyncFileDialog::new()
        .set_file_name("neuraldeep.png")
        .add_filter("PNG", &["png"])
        .save_file()
        .await;
    match file {
        Some(h) => {
            let p = h.path().to_path_buf();
            std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
            Ok(Some(p.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

/// Write a `data:` image to a temp file and open it in the default viewer.
#[tauri::command]
fn open_image(data_url: String) -> Result<(), String> {
    let bytes = decode_data_url(&data_url).ok_or("bad data url")?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("nd-image-{nanos}.png"));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    std::process::Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Write `field: value` (top-level if section=None, else under `section:`) into config.yaml.
fn write_yaml_field(section: Option<&str>, field: &str, value: &str) {
    let path = hermes_home().join("config.yaml");
    let Ok(txt) = std::fs::read_to_string(&path) else { return };
    let mut out = String::new();
    let mut in_section = section.is_none();
    let mut done = false;
    for line in txt.lines() {
        let trimmed = line.trim_start();
        if let Some(sec) = section {
            if !line.starts_with(' ') && line.contains(':') {
                in_section = trimmed.starts_with(&format!("{sec}:"));
            }
        }
        let is_target = if section.is_none() {
            !line.starts_with(' ') && trimmed.starts_with(&format!("{field}:"))
        } else {
            in_section && line.starts_with(' ') && trimmed.starts_with(&format!("{field}:"))
        };
        if is_target && !done {
            let indent = &line[..line.len() - trimmed.len()];
            out.push_str(&format!("{indent}{field}: {value}\n"));
            done = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if done {
        let _ = std::fs::write(&path, out);
    }
}

/// Apply config.yaml updates: keys are "field" (top-level) or "section.field".
#[tauri::command]
fn set_config(updates: std::collections::HashMap<String, String>) -> Result<(), String> {
    for (k, v) in updates {
        match k.split_once('.') {
            Some((sec, field)) => write_yaml_field(Some(sec), field, &v),
            None => write_yaml_field(None, &k, &v),
        }
    }
    Ok(())
}

/// Restart the Hermes gateway so config changes take effect.
#[tauri::command]
fn restart_backend(backend: State<'_, Backend>, config: State<'_, Config>) -> Result<(), String> {
    if let Some(mut c) = backend.child.lock().unwrap().take() {
        let _ = c.kill();
    }
    // Kill any gateway and WAIT until :8642 is actually down — otherwise
    // ensure_backend would reuse the survivor and skip the fresh (sandboxed) spawn.
    for _ in 0..30 {
        let _ = std::process::Command::new("pkill")
            .args(["-f", "hermes gateway"])
            .status();
        if tauri::async_runtime::block_on(fetch_health()).is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    ensure_backend(backend.inner(), config.inner());
    Ok(())
}

/// Copy text to the system clipboard (pbcopy — reliable in the WebView).
#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    use std::io::Write;
    let mut child = std::process::Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .as_mut()
        .ok_or("no stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
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
    // active model (config.yaml model.default) + its context window from cli/status models[]
    let active_model = read_yaml_field("model", "default").unwrap_or_default();
    let ctx = json["models"]
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .find(|m| m["id"].as_str() == Some(active_model.as_str()))
                .and_then(|m| m["ctx"].as_u64())
        })
        .unwrap_or(0);
    Ok(serde_json::json!({
        "tier": json["tier"],
        "user": json["user"]["name"],
        "email": json["user"]["email"],
        "rpm": json["limits"]["rpm"],
        "parallel": json["limits"]["parallel"],
        "models": json["models"].as_array().map(|a| a.len()).unwrap_or(0),
        "model": active_model,
        "ctx": ctx,
        "model_list": json["models"],
    }))
}

/// Read a `field:` under a top-level `section:` from ~/.hermes/config.yaml (best-effort).
fn read_yaml_field(section: &str, field: &str) -> Option<String> {
    let txt = std::fs::read_to_string(hermes_home().join("config.yaml")).ok()?;
    let mut in_section = false;
    for line in txt.lines() {
        let trimmed = line.trim_start();
        if !line.starts_with(' ') && line.contains(':') {
            in_section = trimmed.starts_with(&format!("{section}:"));
        }
        if in_section && trimmed.starts_with(&format!("{field}:")) {
            return Some(trimmed[field.len() + 1..].trim().to_string());
        }
    }
    None
}

/// Live usage/limits from the hub (gate.session/week + rolling usage + rouble wallet).
#[tauri::command]
async fn usage(config: State<'_, Config>) -> Result<serde_json::Value, String> {
    let key = config.hub_key.clone();
    if key.is_empty() {
        return Err("no hub key".into());
    }
    let resp = reqwest::Client::new()
        .get("https://hub.neuraldeep.ru/api/cli/usage")
        .header("Authorization", format!("Bearer {key}"))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    resp.json().await.map_err(|e| e.to_string())
}

/// Generate an image via the hub Image API (FLUX, async: submit → poll → result).
/// Returns a `data:image/png;base64,…` URL.
#[tauri::command]
async fn generate_image(
    prompt: String,
    aspect: Option<String>,
    config: State<'_, Config>,
) -> Result<String, String> {
    use base64::Engine;
    let key = config.hub_key.clone();
    if key.is_empty() {
        return Err("no hub key".into());
    }
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {key}");

    // 1. submit
    let submit = client
        .post(format!("{HUB_IMG}/generate"))
        .header("Authorization", &bearer)
        .json(&serde_json::json!({
            "prompt": prompt,
            "options": { "aspect_ratio": aspect.unwrap_or_else(|| "1:1".into()) },
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let sj: serde_json::Value = submit.json().await.map_err(|e| e.to_string())?;
    let uid = sj["task_uid"]
        .as_str()
        .ok_or_else(|| format!("no task_uid: {sj}"))?
        .to_string();

    // 2. poll (up to ~90s)
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let st = client
            .get(format!("{HUB_IMG}/tasks/{uid}"))
            .header("Authorization", &bearer)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| e.to_string())?;
        match st["status"].as_str().unwrap_or("") {
            "finished" => break,
            "failed" | "error" => return Err(format!("image task failed: {st}")),
            _ => continue,
        }
    }

    // 3. result (binary)
    let bytes = client
        .get(format!("{HUB_IMG}/tasks/{uid}/result"))
        .header("Authorization", &bearer)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{b64}"))
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
            usage,
            delete_session,
            rename_session,
            generate_title,
            pick_workspace,
            copy_text,
            workspace_diff,
            generate_image,
            save_image,
            open_image,
            set_config,
            set_sandbox,
            restart_backend,
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
