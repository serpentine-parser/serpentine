"""User-extensible hook registry for MCP auth, modeled on cube.js's @config(name) pattern.

Users author a Python file (default: ./serpentine_mcp_config.py, override via
SERPENTINE_MCP_CONFIG_FILE) that decorates functions with @config("hook_name").
Importing that file registers the hooks as a side effect; callers then look
them up with get_hook().
"""

import importlib.util
import logging
from pathlib import Path
from typing import Callable, TypeVar

logger = logging.getLogger(__name__)

F = TypeVar("F", bound=Callable)

_HOOKS: dict[str, Callable] = {}


def config(name: str) -> Callable[[F], F]:
    """Decorator: register a function as the implementation of hook `name`."""

    def decorator(fn: F) -> F:
        _HOOKS[name] = fn
        return fn

    return decorator


def get_hook(name: str) -> Callable | None:
    return _HOOKS.get(name)


def load_user_config(path: Path) -> None:
    """Import a user config file so its @config(...) decorators register hooks."""
    spec = importlib.util.spec_from_file_location("serpentine_mcp_user_config", path)
    if spec is None or spec.loader is None:
        raise ImportError(f"Could not load MCP config file: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
