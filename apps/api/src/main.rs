use std::{env, fs, path::{Path, PathBuf}, process::Command, sync::Arc};

use anyhow::{anyhow, Context, Result};
use axum::{
    extract::{Path as AxPath, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    workspace_root: PathBuf,
    inference_base_url: String,
    inference_model: String,
    inference_api_key: String,
    http: Client,
    db_lock: Arc<Mutex<()>>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct CreateProjectRequest {
    name: String,
    template: String,
}

#[derive(Serialize)]
struct CreateProjectResponse {
    project_id: String,
    path: String,
}

#[derive(Serialize, Deserialize)]
struct GenerateRequest {
    prompt: String,
}

#[derive(Serialize)]
struct GenerateResponse {
    generation_id: String,
    files_written: usize,
}

#[derive(Serialize)]
struct PipelineResponse {
    fmt_ok: bool,
    check_ok: bool,
    test_ok: bool,
    clippy_ok: bool,
    logs: Vec<StepLog>,
}

#[derive(Serialize)]
struct StepLog {
    step: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Serialize, Deserialize)]
struct ChatRequest {
    message: String,
}

#[derive(Serialize)]
struct ChatResponse {
    response: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let root = env::current_dir()?.join("workspace_data");
    fs::create_dir_all(&root)?;
    let db_path = root.join("rustcoder.db");
    let projects_root = root.join("projects");
    fs::create_dir_all(&projects_root)?;

    init_db(&db_path)?;

    let state = AppState {
        db_path,
        workspace_root: projects_root,
        inference_base_url: env::var("RUSTCODER_INFERENCE_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8000/v1".into()),
        inference_model: env::var("RUSTCODER_INFERENCE_MODEL")
            .unwrap_or_else(|_| "Fortytwo-Network/Strand-Rust-Coder-14B-v1".into()),
        inference_api_key: env::var("RUSTCODER_INFERENCE_API_KEY").unwrap_or_else(|_| "placeholder".into()),
        http: Client::new(),
        db_lock: Arc::new(Mutex::new(())),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/v1/health", get(health))
        .route("/api/v1/templates", get(list_templates))
        .route("/api/v1/projects", post(create_project))
        .route("/api/v1/projects/:id/generate", post(generate_code))
        .route("/api/v1/projects/:id/pipeline/run", post(run_pipeline))
        .route("/api/v1/chat", post(chat))
        .with_state(state);

    let addr = env::var("RUSTCODER_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("rustcoder-api listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../web/index.html"))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        timestamp: Utc::now(),
    })
}

async fn list_templates() -> Json<Value> {
    Json(json!([
        {"id":"cli-basic","name":"Rust CLI"},
        {"id":"axum-basic","name":"Axum API"}
    ]))
}

async fn create_project(
    State(state): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<CreateProjectResponse>, (StatusCode, String)> {
    let project_id = Uuid::new_v4().to_string();
    let project_slug = req.name.to_lowercase().replace(' ', "-");
    let dir = state.workspace_root.join(format!("{}-{}", project_slug, &project_id[..8]));

    apply_template(&req.template, &req.name, &dir).map_err(internal_err)?;

    {
        let _guard = state.db_lock.lock().await;
        let conn = Connection::open(&state.db_path).map_err(internal_err)?;
        conn.execute(
            "INSERT INTO projects (id, name, template, path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project_id, req.name, req.template, dir.to_string_lossy(), Utc::now().to_rfc3339()],
        )
        .map_err(internal_err)?;
    }

    Ok(Json(CreateProjectResponse {
        project_id,
        path: dir.to_string_lossy().to_string(),
    }))
}

async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, String)> {
    let response = call_strand_chat(&state, &req.message).await.map_err(internal_err)?;
    Ok(Json(ChatResponse { response }))
}

async fn generate_code(
    State(state): State<AppState>,
    AxPath(project_id): AxPath<String>,
    Json(req): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    let project_path = get_project_path(&state, &project_id).await.map_err(internal_err)?;
    let generation_id = Uuid::new_v4().to_string();

    let instruction = format!(
        "Return ONLY valid JSON with this schema: {{\"files\":[{{\"path\":\"src/main.rs\",\"content\":\"...\"}}]}}.\nTask: {}\nAll code must compile.",
        req.prompt
    );

    let raw = call_strand_chat(&state, &instruction).await.map_err(internal_err)?;
    let parsed: Value = serde_json::from_str(&raw)
        .or_else(|_| extract_json(&raw).and_then(|s| serde_json::from_str(&s).map_err(|e| anyhow!(e))))
        .map_err(internal_err)?;

    let files = parsed
        .get("files")
        .and_then(|v| v.as_array())
        .ok_or_else(|| internal_err(anyhow!("model output missing files[]")))?;

    let mut count = 0usize;
    for f in files {
        let path = f.get("path").and_then(|v| v.as_str()).ok_or_else(|| internal_err(anyhow!("missing path")))?;
        let content = f.get("content").and_then(|v| v.as_str()).ok_or_else(|| internal_err(anyhow!("missing content")))?;
        let target = project_path.join(path);
        if !target.starts_with(&project_path) {
            return Err((StatusCode::BAD_REQUEST, "invalid file path".into()));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(internal_err)?;
        }
        fs::write(target, content).map_err(internal_err)?;
        count += 1;
    }

    Ok(Json(GenerateResponse {
        generation_id,
        files_written: count,
    }))
}

async fn run_pipeline(
    State(state): State<AppState>,
    AxPath(project_id): AxPath<String>,
) -> Result<Json<PipelineResponse>, (StatusCode, String)> {
    let path = get_project_path(&state, &project_id).await.map_err(internal_err)?;

    let steps = vec![
        ("fmt", vec!["fmt"]),
        ("check", vec!["check"]),
        ("test", vec!["test"]),
        ("clippy", vec!["clippy", "--", "-D", "warnings"]),
    ];

    let mut logs = Vec::new();
    let mut fmt_ok = false;
    let mut check_ok = false;
    let mut test_ok = false;
    let mut clippy_ok = false;

    for (step, args) in steps {
        let output = Command::new("cargo")
            .args(args)
            .current_dir(&path)
            .output()
            .map_err(internal_err)?;

        let code = output.status.code().unwrap_or(1);
        let log = StepLog {
            step: step.into(),
            exit_code: code,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        };

        match step {
            "fmt" => fmt_ok = code == 0,
            "check" => check_ok = code == 0,
            "test" => test_ok = code == 0,
            "clippy" => clippy_ok = code == 0,
            _ => {}
        }

        logs.push(log);
    }

    Ok(Json(PipelineResponse {
        fmt_ok,
        check_ok,
        test_ok,
        clippy_ok,
        logs,
    }))
}

fn apply_template(template: &str, project_name: &str, target: &Path) -> Result<()> {
    if target.exists() {
        return Err(anyhow!("target already exists"));
    }
    fs::create_dir_all(target)?;

    let src = match template {
        "cli-basic" => PathBuf::from("templates/cli-basic"),
        "axum-basic" => PathBuf::from("templates/axum-basic"),
        _ => return Err(anyhow!("unknown template")),
    };

    copy_dir_all(&src, target)?;

    let cargo_toml = target.join("Cargo.toml");
    let current = fs::read_to_string(&cargo_toml)?;
    fs::write(
        &cargo_toml,
        current.replace("__PROJECT_NAME__", &sanitize_crate_name(project_name)),
    )?;

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(src)?;
        let target = dst.join(rel);
        if path.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &target)?;
        }
    }
    Ok(())
}

fn sanitize_crate_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

async fn get_project_path(state: &AppState, project_id: &str) -> Result<PathBuf> {
    let _guard = state.db_lock.lock().await;
    let conn = Connection::open(&state.db_path)?;
    let mut stmt = conn.prepare("SELECT path FROM projects WHERE id = ?1")?;
    let path: String = stmt.query_row(params![project_id], |row| row.get(0))?;
    Ok(PathBuf::from(path))
}

async fn call_strand_chat(state: &AppState, user_prompt: &str) -> Result<String> {
    let url = format!("{}/chat/completions", state.inference_base_url.trim_end_matches('/'));
    let body = json!({
        "model": state.inference_model,
        "messages": [
            {"role": "system", "content": "You are RustCoder backend model. Produce practical production-grade Rust code."},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.2
    });

    let res = state
        .http
        .post(url)
        .bearer_auth(&state.inference_api_key)
        .json(&body)
        .send()
        .await
        .context("inference request failed")?;

    let status = res.status();
    let v: Value = res.json().await.context("invalid inference json")?;
    if !status.is_success() {
        return Err(anyhow!("inference error: {status} {v}"));
    }

    let text = v
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|x| x.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow!("invalid completion schema"))?;

    Ok(text.to_string())
}

fn extract_json(raw: &str) -> Result<String> {
    let start = raw.find('{').ok_or_else(|| anyhow!("no json start"))?;
    let end = raw.rfind('}').ok_or_else(|| anyhow!("no json end"))?;
    if end <= start {
        return Err(anyhow!("invalid json bounds"));
    }
    Ok(raw[start..=end].to_string())
}

fn init_db(path: &Path) -> Result<()> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            template TEXT NOT NULL,
            path TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}

fn internal_err<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}
