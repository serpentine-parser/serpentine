"""Tests for the @config(...) hook registry used by MCP auth extensibility."""

import pytest

from serpentine.mcp import config as mcp_config


def test_config_decorator_registers_hook():
    @mcp_config.config("check_auth")
    def my_hook(claims):
        return True

    assert mcp_config.get_hook("check_auth") is my_hook


def test_get_hook_returns_none_when_unregistered():
    assert mcp_config.get_hook("check_auth") is None


def test_load_user_config_runs_decorators(tmp_path):
    config_file = tmp_path / "serpentine_mcp_config.py"
    config_file.write_text(
        "from serpentine.mcp.config import config\n"
        "\n"
        "@config('check_auth')\n"
        "def check_auth(claims):\n"
        "    return claims.get('org_id') == 'acme'\n"
    )
    mcp_config.load_user_config(config_file)

    hook = mcp_config.get_hook("check_auth")
    assert hook is not None
    assert hook({"org_id": "acme"}) is True
    assert hook({"org_id": "other"}) is False


def test_load_user_config_propagates_syntax_errors(tmp_path):
    config_file = tmp_path / "broken_config.py"
    config_file.write_text("def broken(:\n")
    with pytest.raises(SyntaxError):
        mcp_config.load_user_config(config_file)


def test_load_user_config_propagates_import_errors(tmp_path):
    config_file = tmp_path / "broken_import.py"
    config_file.write_text("import this_module_does_not_exist\n")
    with pytest.raises(ImportError):
        mcp_config.load_user_config(config_file)
