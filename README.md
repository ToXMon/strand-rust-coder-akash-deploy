# RustCoder

RustCoder is a developer platform that generates production Rust code using a self-hosted **Rust Strand Coder** model deployed on **Akash**.

This branch (`feature/rustcoder-autonomous-platform`) transforms the original deployment repo into a working MVP with:

- Web chat interface
- Project generation from Rust templates
- AI code generation through Akash-hosted Rust Strand Coder
- Compile/test/lint/format automation pipeline
- Local persistence (SQLite)

---

## Repository Layout

```text
.
├── apps/api/                    # Rust Axum backend API
├── templates/
│   ├── cli-basic/               # Starter Rust CLI template
│   └── axum-basic/              # Starter Axum API template
├── web/index.html               # Minimal frontend UI served by backend
├── deploy.yaml                  # Akash SDL for vLLM + Rust Strand Coder
├── Cargo.toml                   # Workspace manifest
└── README.md
```

---

## Architecture (MVP)

- **Frontend**: static HTML/JS (`web/index.html`)
- **Backend**: Axum server (`apps/api`)
- **AI Engine**: OpenAI-compatible HTTP call to Akash vLLM endpoint
- **Database**: SQLite (`workspace_data/rustcoder.db`)
- **Project Workspaces**: generated under `workspace_data/projects/`

Pipeline per generated project:

1. `cargo fmt`
2. `cargo check`
3. `cargo test`
4. `cargo clippy -- -D warnings`

---

## Prerequisites

- Rust toolchain (`cargo`, `rustfmt`, `clippy`)
- Git
- Curl
- (Optional) `gh` CLI for GitHub operations

If Rust is not installed:

```bash
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env"
rustup component add rustfmt clippy
```

---

## Run Locally

### 1) Clone and checkout branch

```bash
git clone https://github.com/ToXMon/strand-rust-coder-akash-deploy.git
cd strand-rust-coder-akash-deploy
git checkout feature/rustcoder-autonomous-platform
```

### 2) Configure model endpoint (Akash)

```bash
export RUSTCODER_INFERENCE_BASE_URL="http://<akash-uri>/v1"
export RUSTCODER_INFERENCE_MODEL="Fortytwo-Network/Strand-Rust-Coder-14B-v1"
export RUSTCODER_INFERENCE_API_KEY="placeholder"
```

> If your endpoint does not require auth, keep a placeholder value.

### 3) Build

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build
```

### 4) Run API

```bash
cargo run -p rustcoder-api
```

Server starts on `0.0.0.0:8080` by default.

Open UI:

- `http://localhost:8080`

Health check:

```bash
curl http://localhost:8080/api/v1/health
```

---

## API Endpoints (MVP)

- `GET /api/v1/health`
- `GET /api/v1/templates`
- `POST /api/v1/projects`
- `POST /api/v1/projects/:id/generate`
- `POST /api/v1/projects/:id/pipeline/run`
- `POST /api/v1/chat`

### Create project

```bash
curl -X POST http://localhost:8080/api/v1/projects \
  -H 'content-type: application/json' \
  -d '{"name":"my app","template":"cli-basic"}'
```

### Generate code for project

```bash
curl -X POST http://localhost:8080/api/v1/projects/<PROJECT_ID>/generate \
  -H 'content-type: application/json' \
  -d '{"prompt":"build a simple CLI with argument parsing"}'
```

### Run compile pipeline

```bash
curl -X POST http://localhost:8080/api/v1/projects/<PROJECT_ID>/pipeline/run
```

---

## Deploy Rust Strand Coder on Akash (Model Backend)

This repo includes `deploy.yaml` for vLLM with model:

- `Fortytwo-Network/Strand-Rust-Coder-14B-v1`

### 1) Deploy (summary)

```bash
provider-services tx deployment create deploy.yaml --from <wallet> --node https://rpc.akashnet.net:443 --chain-id akashnet-2 --gas auto --gas-adjustment 1.4 --yes
provider-services query market bid list --owner $(provider-services keys show <wallet> -a) --node https://rpc.akashnet.net:443 --chain-id akashnet-2
provider-services tx market lease create --dseq <DSEQ> --gseq 1 --oseq 1 --provider <PROVIDER> --from <wallet> --node https://rpc.akashnet.net:443 --chain-id akashnet-2 --gas auto --gas-adjustment 1.4 --yes
provider-services send-manifest deploy.yaml --dseq <DSEQ> --from <wallet> --provider <PROVIDER>
```

### 2) Get endpoint URI

```bash
provider-services lease-status --dseq <DSEQ> --gseq 1 --oseq 1 --provider <PROVIDER>
```

Use returned URI in RustCoder:

```bash
export RUSTCODER_INFERENCE_BASE_URL="http://<akash-uri>/v1"
```

### 3) Validate model endpoint directly

```bash
curl http://<akash-uri>/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "Fortytwo-Network/Strand-Rust-Coder-14B-v1",
    "messages": [
      {"role": "system", "content": "You are a strict Rust coding assistant."},
      {"role": "user", "content": "Write a Rust function with tests."}
    ],
    "temperature": 0.2
  }'
```

---

## Production Deployment (RustCoder App + Akash Model)

Recommended split:

1. **Inference plane (Akash):** host Rust Strand Coder with `deploy.yaml`
2. **App plane (VM/container):** run `rustcoder-api` behind reverse proxy

### Environment variables (production)

```bash
RUSTCODER_BIND=0.0.0.0:8080
RUSTCODER_INFERENCE_BASE_URL=http://<akash-uri>/v1
RUSTCODER_INFERENCE_MODEL=Fortytwo-Network/Strand-Rust-Coder-14B-v1
RUSTCODER_INFERENCE_API_KEY=placeholder
RUST_LOG=info
```

### Build release

```bash
cargo build --release -p rustcoder-api
```

### Run release binary

```bash
./target/release/rustcoder-api
```

### Reverse proxy (Nginx example)

```nginx
server {
  listen 80;
  server_name rustcoder.example.com;

  location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
  }
}
```

---

## Operational Notes

- Generated projects are written to `workspace_data/projects/`.
- SQLite DB is at `workspace_data/rustcoder.db`.
- For long-term production, move to PostgreSQL and add auth + per-user isolation.
- If model output contains invalid JSON, generation endpoint currently tries JSON extraction fallback.

---

## Current MVP Status

Implemented and working:

- Backend API: complete for MVP scope
- Frontend: complete for MVP scope
- AI connectivity: OpenAI-compatible Akash endpoint wired
- Pipeline: fmt/check/test/clippy execution wired and validated

---

## Branch

- `feature/rustcoder-autonomous-platform`
- Commit: `7b2cfe7`
