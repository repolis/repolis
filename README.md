# Repolis

Shows a code repository as a 3D city. Supports C, Rust and Go.

## Requirements

- Go 1.26, Node 22, git
- Rust through rustup (version pinned in `engine/rust-toolchain.toml`) and wasm-pack
- A browser with WebGPU
- Optional: an OpenAI-compatible LLM endpoint, such as llama.cpp's
  `llama-server`, to name districts. Without one, districts are named after
  their main directory.

## Run

```bash
cp backend/.env.example backend/.env
cd frontend && npm ci
make dev
```

Open http://localhost:5173/city/github.com/sqlite/sqlite. Any public GitHub,
GitLab, Codeberg or Bitbucket repository works the same way.

`docker compose up --build` runs everything on http://localhost:3000.

## Develop

- `make dev` spins up a dev server of both frontend and backend.
- `make check` runs the same checks as CI.
- `make wasm` builds the release engine. `npm run dev` builds a faster,
  unoptimised one when `engine/` changed.
- `cd backend && go run ./cmd/analyze <path>` prints what the analyzer extracts
  from a local checkout, no server needed.

```
backend/    Go: clone, parse, call graph, git history, clustering, API
engine/     Rust + Bevy, compiled to WebAssembly: layout and rendering
frontend/   React + Vite: UI around the canvas
```
