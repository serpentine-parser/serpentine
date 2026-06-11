"""Module source dependency edges — `source = "./modules/vpc"` style."""
from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge, assert_no_edge


def test_local_module_source():
    """module block with a local path source emits an import edge to the sourced module."""
    root = dedent("""\
        module "vpc" {
          source = "./modules/vpc"
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", root)])
    assert_has_edge(edges, "main", "modules.vpc", "imports")


def test_nested_local_module_source():
    """Nested local path source is normalized to dotted path."""
    root = dedent("""\
        module "db" {
          source = "./modules/db/postgres"
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", root)])
    assert_has_edge(edges, "main", "modules.db.postgres", "imports")


def test_registry_module_suppressed():
    """Registry module sources (hashicorp/consul/aws) produce a third-party node, not a local import."""
    root = dedent("""\
        module "consul" {
          source  = "hashicorp/consul/aws"
          version = "0.1.0"
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", root)])
    # Should not produce a local import edge — registry modules are third-party
    assert_no_edge(edges, "main", "hashicorp.consul.aws", "imports")


def test_multiple_modules():
    """Multiple module blocks each produce their own import edge."""
    root = dedent("""\
        module "vpc" {
          source = "./modules/vpc"
        }
        module "sg" {
          source = "./modules/security_group"
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", root)])
    assert_has_edge(edges, "main", "modules.vpc", "imports")
    assert_has_edge(edges, "main", "modules.security_group", "imports")
