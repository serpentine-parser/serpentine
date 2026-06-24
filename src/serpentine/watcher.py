"""
File watching module for detecting source code changes.

Responsibilities:
- Monitoring a directory for file changes
- Debouncing rapid changes
- Triggering callbacks on relevant file changes

This module is decoupled from the graph analysis - it only knows
when to notify that files have changed, not what to do about it.
"""

import logging
import threading
from collections.abc import Callable
from pathlib import Path

from watchdog.events import FileMovedEvent, FileSystemEvent, FileSystemEventHandler
from watchdog.observers import Observer

logger = logging.getLogger(__name__)

# Directories to ignore
IGNORED_DIRECTORIES: set[str] = {
    "__pycache__",
    ".git",
    ".venv",
    "venv",
    "node_modules",
    ".mypy_cache",
    ".pytest_cache",
    ".tox",
    "dist",
    "build",
    "egg-info",
    "target",
}


class FileWatcher:
    """
    Watches a directory for source file changes and triggers callbacks.

    Uses debouncing to avoid triggering multiple times for rapid
    successive changes (e.g., when an IDE saves multiple files).

    Usage:
        def on_change():
            print("Files changed!")

        watcher = FileWatcher(Path("./my-project"), on_change)
        watcher.start()

        # ... later ...
        watcher.stop()

    The watcher runs in a background thread and is non-blocking.
    """

    def __init__(
        self,
        path: Path,
        on_change: Callable[[dict[str, str]], None],
        debounce_seconds: float = 0.5,
        extensions: set[str] | None = None,
    ) -> None:
        """
        Initialize the file watcher.

        Args:
            path: Directory to watch
            on_change: Callback invoked with {path: event_type} when relevant files change
            debounce_seconds: Minimum time between callback invocations
            extensions: File extensions to watch (e.g. {".py", ".ts"}).
                        Defaults to all extensions supported by the analyzer.
        """
        self._path = path
        self._on_change = on_change
        self._debounce_seconds = debounce_seconds
        self._extensions = extensions or {".py", ".js", ".jsx", ".ts", ".tsx", ".rs", ".tf"}

        self._observer: Observer | None = None  # type: ignore
        self._handler = _DebouncedEventHandler(
            callback=self._trigger_change,
            debounce_seconds=debounce_seconds,
            extensions=self._extensions,
        )
        self._running = False

    def start(self) -> None:
        """Start watching for file changes."""
        if self._running:
            return

        observer = Observer()
        observer.schedule(
            self._handler,
            str(self._path),
            recursive=True,
        )
        observer.start()
        self._observer = observer
        self._running = True
        logger.info(f"Started watching: {self._path}")

    def stop(self) -> None:
        """Stop watching for file changes."""
        if not self._running or self._observer is None:
            return

        self._observer.stop()
        self._observer.join(timeout=2.0)
        self._observer = None
        self._running = False
        logger.info("Stopped file watcher")

    def _trigger_change(self, changed_files: dict[str, str]) -> None:
        """Internal method to trigger the change callback."""
        logger.debug("File change detected, triggering callback")
        try:
            self._on_change(changed_files)
        except Exception as e:
            logger.error(f"Error in file change callback: {e}")

    @property
    def is_running(self) -> bool:
        """Whether the watcher is currently active."""
        return self._running


class _DebouncedEventHandler(FileSystemEventHandler):
    """
    File system event handler with debouncing.

    Collects events and only triggers the callback after a quiet
    period with no new events. This handles the common case of
    IDEs saving multiple files in quick succession.
    """

    def __init__(
        self,
        callback: Callable[[dict[str, str]], None],
        debounce_seconds: float,
        extensions: set[str],
    ) -> None:
        super().__init__()
        self._callback = callback
        self._debounce_seconds = debounce_seconds
        self._extensions = extensions
        self._last_event_time: float = 0
        self._pending_callback: threading.Timer | None = None
        self._lock = threading.Lock()
        self._pending_files: dict[str, str] = {}

    def on_any_event(self, event: FileSystemEvent) -> None:
        """Handle any file system event."""
        dest = getattr(event, "dest_path", None)
        logger.debug(
            f"[watcher] {event.__class__.__name__} type={event.event_type}"
            f" src={event.src_path}" + (f" dest={dest}" if dest else "")
        )
        if event.is_directory:
            return
        if isinstance(event, FileMovedEvent):
            self._handle_move(event)
            return

        path = Path(str(event.src_path))
        if path.suffix not in self._extensions:
            return
        if any(ignored in path.parts for ignored in IGNORED_DIRECTORIES):
            return

        logger.debug(f"Relevant file event: {event.event_type} {path.name}")
        existing = self._pending_files.get(str(path))
        if existing in ("created", "modified"):
            pass  # preserve: spurious delete after atomic replace must not downgrade
        elif event.event_type == "created" and existing == "deleted":
            self._pending_files[str(path)] = "modified"
        else:
            self._pending_files[str(path)] = event.event_type
        self._schedule_callback()

    def _handle_move(self, event: FileMovedEvent) -> None:
        triggered = False
        src = Path(str(event.src_path))
        dest = Path(str(event.dest_path))

        if src.suffix in self._extensions and not any(
            ignored in src.parts for ignored in IGNORED_DIRECTORIES
        ):
            if self._pending_files.get(str(src)) != "created":
                self._pending_files[str(src)] = "deleted"
            triggered = True

        if dest.suffix in self._extensions and not any(
            ignored in dest.parts for ignored in IGNORED_DIRECTORIES
        ):
            logger.debug(f"Atomic file replacement: {dest.name}")
            if self._pending_files.get(str(dest)) != "created":
                self._pending_files[str(dest)] = "modified"
            triggered = True

        if triggered:
            self._schedule_callback()

    def _schedule_callback(self) -> None:
        """Schedule the callback with debouncing."""
        with self._lock:
            # Cancel any pending callback
            if self._pending_callback is not None:
                self._pending_callback.cancel()

            # Schedule new callback after debounce period
            self._pending_callback = threading.Timer(
                self._debounce_seconds,
                self._execute_callback,
            )
            self._pending_callback.start()

    def _execute_callback(self) -> None:
        """Execute the callback after debounce period."""
        with self._lock:
            changed_files = dict(self._pending_files)
            self._pending_files.clear()
            self._pending_callback = None

        logger.debug(f"[watcher] firing callback: {changed_files}")
        self._callback(changed_files)
