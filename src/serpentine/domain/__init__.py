"""
Serpentine domain layer.

Re-exports all pure domain logic: graph operations, selectors, config, and services.
Adapters (CLI, HTTP API, MCP) should import from here, not from individual modules.
"""

from serpentine.config import Config
from serpentine.selector import GraphSelector, filter_by_state
from serpentine.services import (
    MissingConfigError,
    NotIngestedError,
    SourceProvider,
    UnknownRepoError,
    apply_filters,
    filter_by_origin,
    get_catalog,
    get_graph,
    get_stats,
    ingest_ref,
    inject_source,
    inject_source_on_demand,
)

__all__ = [
    # Config
    "Config",
    # Graph query
    "GraphSelector",
    "filter_by_state",
    "filter_by_origin",
    "apply_filters",
    "get_graph",
    "get_catalog",
    "get_stats",
    "inject_source",
    "inject_source_on_demand",
    # Source provider protocol
    "SourceProvider",
    # Ingestion
    "ingest_ref",
    # Exceptions
    "NotIngestedError",
    "MissingConfigError",
    "UnknownRepoError",
]
