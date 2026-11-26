.PHONY: dev dev-backend dev-frontend wasm check check-backend check-engine check-frontend

AIR = github.com/air-verse/air@v1.67.4

dev-backend:
	cd backend && go run $(AIR)

dev-frontend:
	cd frontend && npm run dev

dev:
	$(MAKE) -j2 dev-backend dev-frontend

wasm:
	engine/build-wasm.sh

# The same commands CI runs, so a local pass means a CI pass.
check: check-backend check-engine check-frontend

check-backend:
	cd backend && test -z "$$(gofmt -l .)" || { gofmt -l .; exit 1; }
	cd backend && go vet ./...
	cd backend && go test -race ./...

check-engine:
	cd engine && cargo fmt --check
	cd engine && cargo clippy --release --all-targets -- -D warnings
	cd engine && cargo test --release

check-frontend:
	cd frontend && npx tsc -b
	cd frontend && npx eslint .
	cd frontend && npx prettier --check .
	cd frontend && npx vitest run
