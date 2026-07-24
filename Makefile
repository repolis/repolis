.PHONY: setup dev dev-backend dev-frontend

CYAN = \033[0;36m
NC = \033[0m

setup:
	@echo "${CYAN}checking dependencies...${NC}"
	@command -v go >/dev/null 2>&1 || { echo >&2 "Go is not installed."; exit 1; }
	@command -v node >/dev/null 2>&1 || { echo >&2 "Node is not installed."; exit 1; }
	@command -v cargo >/dev/null 2>&1 || { echo >&2 "Cargo is not installed."; exit 1; }
	@command -v wasm-pack >/dev/null 2>&1 || { echo >&2 "wasm-pack is not installed."; exit 1; }
	@command -v air >/dev/null 2>&1 || { echo >&2 "installing air..."; go install github.com/air-verse/air@latest; }
	cd backend && go mod tidy
	cd frontend && npm install

dev-backend:
	cd backend && air

dev-frontend:
	cd frontend && npm run dev

dev:
	$(MAKE) -j2 dev-backend dev-frontend
