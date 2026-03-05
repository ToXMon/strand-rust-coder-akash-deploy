# RustCoder MVP (Autonomous Build)

RustCoder is a developer platform that generates production Rust code using a self-hosted **Rust Strand Coder** model deployed on Akash.

This repo now contains:
- Akash model deploy SDL (`deploy.yaml`)
- Rust backend API (`apps/api`) for:
  - web chat interface
  - project generation
  - AI code generation
  - compile/test/clippy/fmt automation
  - Rust templates
- minimal web UI (`web/index.html`)
- Rust templates (`templates/cli-basic`, `templates/axum-basic`)

## Architecture Summary
- **Frontend**: static web UI served by Rust API
- **Backend**: Axum API (Rust)
- **AI Engine**: OpenAI-compatible call to Akash vLLM endpoint
- **Storage**: SQLite (`workspace_data/rustcoder.db`)
- **Workspace**: generated projects under `workspace_data/projects/`

## Configure AI Endpoint (Akash)
Set env vars:

```bash
export RUSTCODER_INFERENCE_BASE_URL="http://<akash-uri>/v1"
export RUSTCODER_INFERENCE_MODEL="Fortytwo-Network/Strand-Rust-Coder-14B-v1"
export RUSTCODER_INFERENCE_API_KEY="placeholder"
```

## Run

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo run -p rustcoder-api
```

Open: `http://localhost:8080`

## API (MVP)
- `GET /api/v1/health`
- `GET /api/v1/templates`
- `POST /api/v1/projects`
- `POST /api/v1/projects/:id/generate`
- `POST /api/v1/projects/:id/pipeline/run`
- `POST /api/v1/chat`

## Example
Create project:

```bash
curl -X POST http://localhost:8080/api/v1/projects \
  -H 'content-type: application/json' \
  -d '{"name":"my app","template":"cli-basic"}'
```

Run pipeline:

```bash
curl -X POST http://localhost:8080/api/v1/projects/<PROJECT_ID>/pipeline/run
```

## Akash deployment (model backend)
Use existing `deploy.yaml` from this repo to deploy Rust Strand Coder model on Akash.
