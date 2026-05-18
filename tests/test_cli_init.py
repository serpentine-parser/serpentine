from click.testing import CliRunner

from serpentine.cli import main


def test_init_scaffolds_claude_command_and_config() -> None:
    runner = CliRunner()

    with runner.isolated_filesystem():
        result = runner.invoke(main, ["init"])

        assert result.exit_code == 0
        assert ".claude/commands/serpentine.md" in result.output
        assert ".serpentine.yml" in result.output

        with open(".claude/commands/serpentine.md") as f:
            command_text = f.read()
        with open(".serpentine.yml") as f:
            config_text = f.read()

        assert "Serpentine CLI — Agent Usage Guide" in command_text
        assert "analysis:" in config_text
        assert ".py" in config_text


def test_init_does_not_overwrite_existing_files() -> None:
    runner = CliRunner()

    with runner.isolated_filesystem():
        runner.invoke(main, ["init"])

        with open(".serpentine.yml", "w") as f:
            f.write("custom: true\n")

        result = runner.invoke(main, ["init"])

        assert result.exit_code == 0
        assert "Exists:" in result.output
        with open(".serpentine.yml") as f:
            assert f.read() == "custom: true\n"


def test_init_force_overwrites_existing_files() -> None:
    runner = CliRunner()

    with runner.isolated_filesystem():
        runner.invoke(main, ["init"])

        with open(".serpentine.yml", "w") as f:
            f.write("custom: true\n")

        result = runner.invoke(main, ["init", "--force"])

        assert result.exit_code == 0
        with open(".serpentine.yml") as f:
            assert "custom: true" not in f.read()
