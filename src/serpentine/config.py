"""
Configuration management for Serpentine.

Supports loading configuration from `.serpentine.yml` or `serpentine.yml`
in the project root, with sensible defaults if not found.

Configuration schema:
    analysis:
        extensions: [".py", ".js", ".jsx", ".ts", ".tsx", ".rs"]  # File extensions to analyze
        exclude_dirs: [list of directory names]  # Directories to skip
        exclude_patterns: [list of glob patterns] # File patterns to skip
"""

import json
import logging
from pathlib import Path
from typing import Any, Optional

logger = logging.getLogger(__name__)

# Default configuration
DEFAULT_CONFIG = {
    "analysis": {
        "extensions": [".py", ".js", ".jsx", ".ts", ".tsx", ".rs"],
        "exclude_dirs": [
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
            "static",
            ".next",
            ".nuxt",
            "coverage",
            ".egg-info",
            "target",
        ],
        "exclude_patterns": [],
    }
}


class Config:
    """Manages Serpentine configuration."""

    def __init__(self, config_data: dict[str, Any]) -> None:
        """Initialize config with data."""
        self._data = config_data

    @classmethod
    def load(cls, project_path: Path) -> "Config":
        """
        Load configuration from project directory.

        Looks for `.serpentine.yml` or `serpentine.yml` in order.
        Falls back to default config if not found.

        Args:
            project_path: Root directory of the project

        Returns:
            Config instance
        """
        project_path = Path(project_path).resolve()

        # Try loading config files in order of preference
        config_files = [
            project_path / ".serpentine.yml",
            project_path / "serpentine.yml",
        ]

        for config_file in config_files:
            if config_file.exists():
                logger.info(f"Loading config from: {config_file}")
                return cls._load_from_file(config_file)

        logger.debug("No config file found, using defaults")
        return cls(DEFAULT_CONFIG.copy())

    @classmethod
    def _load_from_file(cls, config_file: Path) -> "Config":
        """Load configuration from a YAML file."""
        try:
            import yaml
        except ImportError:
            logger.warning("PyYAML not installed. Install with: pip install pyyaml")
            return Config(DEFAULT_CONFIG.copy())

        try:
            with open(config_file) as f:
                data = yaml.safe_load(f) or {}

            # Merge with defaults to ensure all required keys exist
            config = cls._merge_with_defaults(data)
            return Config(config)

        except Exception as e:
            logger.error(f"Failed to load config from {config_file}: {e}")
            return Config(DEFAULT_CONFIG.copy())

    @staticmethod
    def _merge_with_defaults(user_config: dict[str, Any]) -> dict[str, Any]:
        """Merge user config with defaults, preferring user values."""
        config = DEFAULT_CONFIG.copy()

        if "analysis" in user_config:
            analysis = config.get("analysis", {})
            user_analysis = user_config["analysis"]

            if "extensions" in user_analysis:
                analysis["extensions"] = user_analysis["extensions"]
            if "exclude_dirs" in user_analysis:
                analysis["exclude_dirs"] = user_analysis["exclude_dirs"]
            if "exclude_patterns" in user_analysis:
                analysis["exclude_patterns"] = user_analysis["exclude_patterns"]

            config["analysis"] = analysis

        return config

    @property
    def extensions(self) -> list[str]:
        """File extensions to analyze."""
        return self._data.get("analysis", {}).get("extensions", [])

    @property
    def exclude_dirs(self) -> set[str]:
        """Directories to exclude."""
        return set(self._data.get("analysis", {}).get("exclude_dirs", []))

    @property
    def exclude_patterns(self) -> list[str]:
        """File patterns to exclude."""
        return self._data.get("analysis", {}).get("exclude_patterns", [])

    def to_dict(self) -> dict[str, Any]:
        """Get full config as dictionary."""
        return self._data.copy()

    def to_json(self) -> str:
        """Get full config as JSON string."""
        return json.dumps(self._data, indent=2)
