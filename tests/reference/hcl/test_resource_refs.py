"""Cross-resource reference edges — `aws_subnet.main.id` style traversals."""

from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_resource_attribute_reference():
    """aws_subnet.main.id in a resource attribute emits a references edge."""
    tf = dedent("""\
        resource "aws_subnet" "main" {
          cidr_block = "10.0.1.0/24"
        }

        resource "aws_instance" "app" {
          subnet_id = aws_subnet.main.id
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(
        edges, "resource.aws_instance.app", "resource.aws_subnet.main", "references"
    )


def test_resource_reference_in_interpolation():
    """aws_security_group.sg.id inside ${} interpolation still emits a references edge."""
    tf = dedent("""\
        resource "aws_security_group" "sg" {}

        resource "aws_instance" "app" {
          user_data = "${aws_security_group.sg.id}-init"
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(
        edges,
        "resource.aws_instance.app",
        "resource.aws_security_group.sg",
        "references",
    )


def test_multiple_resource_deps():
    """A resource referencing two other resources produces two edges."""
    tf = dedent("""\
        resource "aws_subnet" "main" {}
        resource "aws_security_group" "sg" {}

        resource "aws_instance" "app" {
          subnet_id              = aws_subnet.main.id
          vpc_security_group_ids = [aws_security_group.sg.id]
        }
    """)
    edges = analyze_sources([("/fixture/main.tf", tf)])
    assert_has_edge(
        edges, "resource.aws_instance.app", "resource.aws_subnet.main", "references"
    )
    assert_has_edge(
        edges,
        "resource.aws_instance.app",
        "resource.aws_security_group.sg",
        "references",
    )
