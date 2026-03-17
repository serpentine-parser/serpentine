.PHONY: dev dev-rebuild frontend-build frontend-dev install

# Start Tilt dev environment with all services running in parallel
dev:
	tilt up

# Build Rust extension (one-time)
dev-rebuild:
	@uv run maturin develop

# Build frontend (one-time)
frontend-build:
	@cd frontend && npm run build

# Run frontend dev server (hot reload)
frontend-dev:
	@cd frontend && npm run dev

# Build frontend + Rust wheel, then install serpentine CLI globally
install: frontend-build
	uv run maturin build --release --out dist
	wheel=$$(ls -t dist/serpentine-*.whl | head -1) && uv tool install --reinstall "$$wheel"
	uv tool update-shell
