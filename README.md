# Repolis

Turns a source repository into a 3D city you can navigate. One building per
type or file module, grouped into districts by how the code actually calls
itself. **C, Rust and Go** are supported; adding a language is one file.

```
frontend/   React + Vite UI, hosts the canvas and the inspector
backend/    Go: clone, parse, call graph, git history, clustering, API
engine/     Rust + Bevy compiled to WebAssembly: layout and rendering
```

## Running it

```bash
# 1. Backend
cd backend
cp .env.example .env          # then point LLM_BASE_URL at your model server
go run ./cmd/api              # http://localhost:8080

# 2. Engine -> wasm (only needed when engine/ changes)
cd ../engine
./build-wasm.sh

# 3. Frontend
cd ../frontend
npm install
npm run dev                   # http://localhost:5173
```

Then open `http://localhost:5173/city/<owner>/<repo>`, for example
`/city/tsoding/nothing`.

The LLM is optional. Without one configured the backend still serves a complete
city; districts are named after their dominant directory instead of by a model.

## Working on it

```bash
# Extraction quality, no server and no LLM (fast loop)
cd backend && go run ./cmd/analyze /path/to/a/checkout

# Same, plus the LLM refinement pass
go run ./cmd/analyze -llm /path/to/a/checkout

# Tests
cd backend && go test ./...
cd engine  && cargo test --release

# Verify the layout invariants against a real generated city
cd engine && cargo run --release --example layout_check -- \
  ../backend/data/cities/<hash>.json
```

`layout_check` asserts that no two buildings overlap, that every building is
inside its own district, that nothing was dropped, and that height carries
signal. It is the headless substitute for looking at the city.

## Adding a language

Everything language-specific lives in `backend/internal/analyzer/lang/`. The
association ladder, call graph, clustering, layout and renderer never name a
tree-sitter node type.

To add one, implement `lang.Language` in a new file and call `Register` from
its `init`. Nine methods:

| Method | Answers |
|---|---|
| `Name`, `Extensions`, `Grammar` | which files, which tree-sitter grammar |
| `Namespace(path)` | which module a file's symbols belong to |
| `Separator` | how qualified names are written (`::`, `.`) |
| `SyntacticMethods` | does the grammar state method ownership? |
| `ImportTargets` | what namespaces one import makes visible |
| `Parse` | types, functions, calls and imports from one file |

`SyntacticMethods` is the important one. Rust's `impl` block, a Go receiver
and a method inside a class body all *state* the owning type, so `Receiver` is
read straight from the tree and nothing is inferred. C cannot express the idea
at all, which is why it alone needs the heuristic ladder and the model that
adjudicates it. Measured:

| | association | LLM calls |
|---|---|---|
| gin (Go, 99 files) | 430 by syntax, 0 guessed | **8**, all district naming |
| ripgrep (Rust, 110 files) | 1443 by syntax, 0 guessed | **13**, all district naming |
| libgit2 (C, 1032 files) | 7124 by rule, 2549 ambiguous | **616** |

A repository may mix languages; each file is parsed by whichever language
claims its extension, and they share one call graph.

## How it reads

| Channel | Meaning |
|---|---|
| Height | number of functions |
| Footprint | number of fields |
| Hue | district purpose (core, data, network, security, …) |
| Saturation | recency — old code fades toward grey |
| Glowing orange roof cap | commit churn; brighter cap = more commits |
| Dark plinth underneath | a file module rather than a type |
| District ground area | amount of code in that district |
| Distance between districts | how tightly they are coupled |

Drag to orbit, scroll to zoom, WASD to pan, click to select, Esc to clear,
`F` for fly mode, `R` to reset. The legend in the bottom right repeats all of
this, along with how much of the analysis was deterministic.

## Regenerating a city

A finished city is cached by repository and commit, so re-opening it is
instant. The **Regenerate** button next to the search box forces the work to
happen again. Three levels, each including the ones above it:

| Level | Redoes | Use it when |
|---|---|---|
| Re-analyse | parsing, association, call graph, clustering, layout | you changed extraction or layout code |
| Re-ask the model | the above, plus every model answer | you changed a prompt or switched models |
| Re-clone | the above, plus deleting and re-downloading the checkout | the working copy is stale or damaged |

The same thing over HTTP:

```bash
curl -X POST localhost:8080/api/analyze   -H 'Content-Type: application/json'   -d '{"repo_url":"https://github.com/tsoding/nothing","refresh":"model"}'
```

`refresh` is `city`, `model` or `clone`; omit it to use the cache. The older
`"force": true` still works and means `city`. Measured on tsoding/nothing:
cached 0.5 s, `city` 0.7 s with no model calls, `model` 2.6 s with 18.

Architecture and design rationale: [`../tech.md`](../tech.md) and
[`../city_generation.md`](../city_generation.md). Roadmap:
[`../steps.md`](../steps.md).
