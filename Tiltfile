# Serpentine Development Environment
# Run all services in parallel with automatic rebuilds

# Run all services in parallel with automatic rebuilds

# ── Rust Extension ───────────────────────────────────────────────────────────
# Rebuilds when Rust or Cargo.toml changes
local_resource(
  'rust-extension',
  'uv run maturin develop',
  deps=['rust/src', 'rust/Cargo.toml', 'pyproject.toml'],
  labels=['rust'],
  trigger_mode=TRIGGER_MODE_AUTO,
)

# ── Frontend ──────────────────────────────────────────────────────────────────
# Dev server with hot reload
API_PORT = os.getenv('SERPENTINE_API_PORT', '8765')
API_HOST = os.getenv('SERPENTINE_API_HOST', '127.0.0.1')
API_URL = "http://%s:%s" % (API_HOST, API_PORT)

local_resource(
  'frontend-dev',
  serve_cmd='cd frontend && NEXT_PUBLIC_API_URL=%s npm run dev' % API_URL,
  deps=[
    'frontend/src',
    'frontend/public',
    'frontend/package.json',
    'frontend/package-lock.json',
  ],
  labels=['frontend'],
)

# ── Python Server ─────────────────────────────────────────────────────────────
# Runs serpentine server with auto-reload on Python source changes
# Restarts when rust-extension completes (so new .so is loaded)
local_resource(
  'python-server',
  serve_cmd='uv run serpentine serve . --no-browser --port %s' % API_PORT,
  deps=['src/serpentine'],
  labels=['server'],
  resource_deps=['rust-extension'],
  allow_parallel=True,
  trigger_mode=TRIGGER_MODE_AUTO,
)

# ── Commands ──────────────────────────────────────────────────────────────────
# Manual buttons for convenience
local_resource(
  'tilt-info',
  'echo "✓ Tilt dev environment ready. Watching rust/, frontend/, and src/serpentine/ for changes."',
  trigger_mode=TRIGGER_MODE_MANUAL,
)
