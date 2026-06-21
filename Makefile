.PHONY: test bench bench-dry dev dev-rebuild frontend-build frontend-dev install

test:
	uv run pytest tests -v

# Run graph build/incremental benchmark. Set REPO= to a large repo (e.g. ~/Code/dbt-core).
# Optional: RUNS=5 N_FILES=1,5,20,50
bench:
	uv run python benchmarks/bench_graph.py $(or $(REPO),.) --runs $(or $(RUNS),3) --n-files $(or $(N_FILES),1,5,20)

# Show file count and extension breakdown without running the full benchmark.
bench-dry:
	uv run python benchmarks/bench_graph.py $(or $(REPO),.) --dry-run

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
	wheel=$$(ls -t dist/serpentine*.whl | head -1) && uv tool install --reinstall "$$wheel"
	uv tool update-shell
