"""Variable reference edges — `var.name` in attribute values and interpolations."""
from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_bare_var_reference():
    """var.ami_id in a resource attribute emits a references edge to the variable definition."""
    tf = dedent("""\
        variable "ami_id" {}

        resource "aws_instance" "app" {
          ami = var.ami_id
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(edges, "resource.aws_instance.app", "variable.ami_id", "references")


def test_var_reference_in_interpolation():
    """${var.env} inside a string interpolation produces the same references edge."""
    tf = dedent("""\
        variable "env" {}

        resource "aws_instance" "app" {
          tags = "${var.env}-app"
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(edges, "resource.aws_instance.app", "variable.env", "references")


def test_var_reference_in_nested_interpolation():
    """Nested interpolation `${module.vpc.outputs["${var.env}_subnet"]}` still resolves var.env."""
    tf = dedent("""\
        variable "env" {}

        resource "aws_instance" "app" {
          subnet_id = "${module.vpc.outputs["${var.env}_subnet"]}"
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(edges, "resource.aws_instance.app", "variable.env", "references")


def test_multiple_var_refs_in_resource():
    """Multiple var.* references in one resource each produce an edge."""
    tf = dedent("""\
        variable "ami_id" {}
        variable "instance_type" {}

        resource "aws_instance" "app" {
          ami           = var.ami_id
          instance_type = var.instance_type
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(edges, "resource.aws_instance.app", "variable.ami_id", "references")
    assert_has_edge(edges, "resource.aws_instance.app", "variable.instance_type", "references")


def test_var_in_module_argument():
    """var.* used as a module argument also emits a references edge."""
    tf = dedent("""\
        variable "environment" {}

        module "vpc" {
          source = "./modules/vpc"
          env    = var.environment
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(edges, "module.vpc", "variable.environment", "references")
