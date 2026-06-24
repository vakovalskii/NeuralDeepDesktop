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
    /// Provisioning state: "" | "reused" | "starting" | "missing" | "error"
    state: Mutex<String>,
}

fn set_backend_state(backend: &Backend, s: &str) {
    *backend.state.lock().unwrap() = s.to_string();
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
/// Our **own**, isolated Hermes home — never the user's `~/.hermes`.
/// `~/.neuraldeep/hermes` (no spaces, so the bash installer is happy).
fn hermes_home() -> std::path::PathBuf {
    home().join(".neuraldeep").join("hermes")
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

/// The `hermes` launcher inside our provisioned venv (None until provisioned).
fn hermes_launcher() -> Option<String> {
    let p = hermes_home().join("hermes-agent/venv/bin/hermes");
    p.exists().then(|| p.to_string_lossy().into_owned())
}

/// True once the first-run installer has produced a runnable Hermes.
fn provisioned() -> bool {
    hermes_launcher().is_some()
}

#[tauri::command]
fn is_provisioned() -> bool {
    provisioned()
}

/// Random hex key (loopback API_SERVER_KEY) from /dev/urandom.
fn gen_key() -> String {
    let mut buf = [0u8; 24];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut buf);
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
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
        .env("HERMES_HOME", hermes_home()) // isolate from the user's ~/.hermes
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
    let key = current_hub_key();
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

/// Holds the current `say` child so playback can be stopped.
#[derive(Default)]
struct Tts(Mutex<Option<Child>>);

/// Speak text via the macOS `say` engine — Milena (ru) for Cyrillic, else Samantha.
#[tauri::command]
fn speak(text: String, tts: State<'_, Tts>) -> Result<(), String> {
    if let Some(mut c) = tts.0.lock().unwrap().take() {
        let _ = c.kill();
    }
    let cyrillic = text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    let voice = if cyrillic { "Milena" } else { "Samantha" };
    let child = Command::new("say")
        .args(["-v", voice, "-r", "210"])
        .arg(&text)
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    *tts.0.lock().unwrap() = Some(child);
    Ok(())
}

/// Stop any in-progress speech.
#[tauri::command]
fn stop_speak(tts: State<'_, Tts>) -> Result<(), String> {
    if let Some(mut c) = tts.0.lock().unwrap().take() {
        let _ = c.kill();
    }
    let _ = Command::new("pkill").arg("-x").arg("say").status();
    Ok(())
}

/// Transcribe recorded audio (data: URL) via the hub Whisper endpoint.
#[tauri::command]
async fn transcribe(audio: String, config: State<'_, Config>) -> Result<String, String> {
    let bytes = decode_data_url(&audio).ok_or("bad audio data url")?;
    let mime = audio
        .split(';')
        .next()
        .and_then(|s| s.strip_prefix("data:"))
        .unwrap_or("audio/webm")
        .to_string();
    let ext = if mime.contains("mp4") || mime.contains("aac") {
        "m4a"
    } else if mime.contains("wav") {
        "wav"
    } else {
        "webm"
    };
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(format!("audio.{ext}"))
        .mime_str(&mime)
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-1")
        .part("file", part);
    let j: serde_json::Value = reqwest::Client::new()
        .post("https://api.neuraldeep.ru/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", current_hub_key()))
        .multipart(form)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(j["text"].as_str().unwrap_or("").trim().to_string())
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
    let key = current_hub_key();
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
    let key = current_hub_key();
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
    let key = current_hub_key();
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

const INSTALL_URL: &str = "https://hermes-agent.nousresearch.com/install.sh";
const HUB_BASE: &str = "https://api.neuraldeep.ru/v1";

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum ProvisionEvent {
    Stage { stage: String },
    Log { line: String },
    Done { ok: bool, message: String },
}

/// Rewrite the top-level `model:` block to point at the hub with the given key.
fn set_model_block(hub_key: &str) {
    let cfg_path = hermes_home().join("config.yaml");
    let model_block = format!(
        "model:\n  default: qwen3.6-35b-a3b\n  provider: custom\n  base_url: {HUB_BASE}\n  api_key: {hub_key}\n"
    );
    let txt = std::fs::read_to_string(&cfg_path).unwrap_or_default();
    let lines: Vec<&str> = txt.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut replaced = false;
    while i < lines.len() {
        if lines[i].starts_with("model:") {
            out.push_str(&model_block);
            i += 1;
            while i < lines.len()
                && (lines[i].is_empty()
                    || lines[i].starts_with(|c: char| c == ' ' || c == '\t' || c == '#'))
            {
                i += 1;
            }
            replaced = true;
            continue;
        }
        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }
    if !replaced {
        out = format!("{model_block}{out}");
    }
    let _ = std::fs::write(&cfg_path, out);
}

/// Read the current hub key from the config (empty until the user signs in).
fn current_hub_key() -> String {
    if let Ok(txt) = std::fs::read_to_string(hermes_home().join("config.yaml")) {
        let mut in_model = false;
        for line in txt.lines() {
            if line.starts_with("model:") {
                in_model = true;
            } else if in_model && !line.is_empty() && !line.starts_with(|c: char| c == ' ' || c == '\t') {
                in_model = false;
            }
            if in_model {
                if let Some(v) = line.trim().strip_prefix("api_key:") {
                    return v.trim().to_string();
                }
            }
        }
    }
    String::new()
}

/// Point the freshly-installed config at the hub (empty key) + set a loopback key.
fn write_hub_config(hub_key: &str) {
    set_model_block(hub_key);
    let env_path = hermes_home().join(".env");
    let mut env_txt = std::fs::read_to_string(&env_path).unwrap_or_default();
    if !env_txt.is_empty() && !env_txt.ends_with('\n') {
        env_txt.push('\n');
    }
    env_txt.push_str(&format!("API_SERVER_KEY={}\n", gen_key()));
    let _ = std::fs::write(&env_path, env_txt);
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum DeviceEvent {
    Code { user_code: String, url: String },
    Done { ok: bool, message: String },
}

/// RFC 8628 device-authorization against the hub: start → open browser + show the
/// user code → poll until the user approves → save the per-device key. App restarts after.
#[tauri::command]
async fn device_login(on_event: Channel<DeviceEvent>) -> Result<(), String> {
    let client = reqwest::Client::new();
    let start: serde_json::Value = client
        .post("https://hub.neuraldeep.ru/api/cli/device/start")
        .json(&serde_json::json!({ "client": "neural-deep-desktop", "scope": "inference" }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let device_code = start["device_code"].as_str().unwrap_or("").to_string();
    if device_code.is_empty() {
        return Err("device/start не вернул код".into());
    }
    let user_code = start["user_code"].as_str().unwrap_or("").to_string();
    let url = start["verification_uri_complete"]
        .as_str()
        .or_else(|| start["verification_uri"].as_str())
        .unwrap_or("https://hub.neuraldeep.ru/app/device")
        .to_string();
    let mut interval = start["interval"].as_u64().unwrap_or(5).max(2);
    let expires = start["expires_in"].as_u64().unwrap_or(900);

    let _ = Command::new("open").arg(&url).spawn();
    on_event
        .send(DeviceEvent::Code { user_code, url })
        .ok();

    let mut elapsed = 0u64;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        elapsed += interval;
        if elapsed > expires {
            on_event.send(DeviceEvent::Done { ok: false, message: "Время вышло — попробуй ещё раз".into() }).ok();
            return Ok(());
        }
        let j: serde_json::Value = client
            .post("https://hub.neuraldeep.ru/api/cli/device/token")
            .json(&serde_json::json!({ "device_code": device_code }))
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(token) = j["access_token"].as_str() {
            set_model_block(token);
            on_event.send(DeviceEvent::Done { ok: true, message: "ok".into() }).ok();
            return Ok(());
        }
        match j["error"].as_str().unwrap_or("") {
            "authorization_pending" => continue,
            "slow_down" => {
                interval += 5;
                continue;
            }
            "access_denied" => {
                on_event.send(DeviceEvent::Done { ok: false, message: "Вход отклонён".into() }).ok();
                return Ok(());
            }
            "expired_token" => {
                on_event.send(DeviceEvent::Done { ok: false, message: "Код истёк — попробуй ещё раз".into() }).ok();
                return Ok(());
            }
            _ => continue,
        }
    }
}

/// Save the user's hub key (after sign-in) into the config. Caller restarts the app.
#[tauri::command]
fn set_hub_key(key: String) -> Result<(), String> {
    let key = key.trim();
    if !key.starts_with("sk-") || key.len() < 12 {
        return Err("Это не похоже на ключ NeuralDeep (должен начинаться с sk-)".into());
    }
    set_model_block(key);
    Ok(())
}

/// Whether the user has signed in (a hub key is present in the config).
#[tauri::command]
fn has_hub_key() -> bool {
    !current_hub_key().is_empty()
}

/// Open a URL in the user's default browser.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    Command::new("open")
        .arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// First-run install: fetch the official installer, run it into our isolated home,
/// then point it at the hub. Streams progress over the channel. App restarts after.
#[tauri::command]
async fn provision(on_event: Channel<ProvisionEvent>) -> Result<(), String> {
    let _ = std::fs::create_dir_all(hermes_home());
    let _ = on_event.send(ProvisionEvent::Stage { stage: "download".into() });

    let script = reqwest::Client::new()
        .get(INSTALL_URL)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("download installer: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let script_path = hermes_home().join("install.sh");
    std::fs::write(&script_path, script).map_err(|e| e.to_string())?;

    let on2 = on_event.clone();
    // No baked key — the user signs in after install (set_hub_key). The model block
    // is written with an empty api_key; the login screen fills it in.
    let hub_key = String::new();
    let res = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        use std::io::{BufRead, BufReader};
        on2.send(ProvisionEvent::Stage { stage: "install".into() }).ok();
        let mut child = Command::new("bash")
            .arg(&script_path)
            .args(["--non-interactive", "--skip-setup", "--skip-browser"])
            .env("HERMES_HOME", hermes_home())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn installer: {e}"))?;

        // drain stderr on a side thread so the pipe never blocks the installer
        let on_err = on2.clone();
        let stderr = child.stderr.take();
        let herr = std::thread::spawn(move || {
            if let Some(e) = stderr {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    on_err.send(ProvisionEvent::Log { line }).ok();
                }
            }
        });
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                on2.send(ProvisionEvent::Log { line }).ok();
            }
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        let _ = herr.join();
        if !status.success() {
            return Err(format!("installer exited: {status}"));
        }
        on2.send(ProvisionEvent::Stage { stage: "configure".into() }).ok();
        write_hub_config(&hub_key);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?;

    match res {
        Ok(()) => {
            on_event.send(ProvisionEvent::Done { ok: true, message: "ok".into() }).ok();
            Ok(())
        }
        Err(e) => {
            on_event.send(ProvisionEvent::Done { ok: false, message: e.clone() }).ok();
            Err(e)
        }
    }
}

/// Relaunch the app (used right after a successful first-run install).
#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

/// Coarse backend lifecycle for the boot screen.
#[tauri::command]
async fn backend_status(backend: State<'_, Backend>) -> Result<serde_json::Value, String> {
    if fetch_health().await.is_some() {
        return Ok(serde_json::json!({ "state": "online" }));
    }
    if !provisioned() {
        return Ok(serde_json::json!({ "state": "needs_provision" }));
    }
    let s = backend.state.lock().unwrap().clone();
    let state = if s.is_empty() { "starting".to_string() } else { s };
    Ok(serde_json::json!({ "state": state }))
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
        .manage(Tts::default())
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
            speak,
            stop_speak,
            transcribe,
            workspace_diff,
            generate_image,
            save_image,
            open_image,
            set_config,
            set_sandbox,
            restart_backend,
            is_provisioned,
            provision,
            restart_app,
            backend_status,
            set_hub_key,
            has_hub_key,
            open_url,
            device_login,
            create_session,
            chat_stream
        ])
        .setup(move |app| {
            let backend = app.state::<Backend>();
            // Only auto-start the gateway once Hermes is provisioned; otherwise the
            // frontend shows the first-run setup screen and drives provisioning.
            if provisioned() {
                ensure_backend(&backend, &cfg);
            } else {
                set_backend_state(&backend, "needs_provision");
            }
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
