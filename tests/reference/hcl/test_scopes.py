"""Scope nodes — top-level blocks appear as nodes in the graph."""

import json
from textwrap import dedent

from serpentine import _analyzer


def _nodes_flat(sources):
    """Return all node IDs (including children) from the graph."""
    fm = _analyzer.FileManager()
    for path, source in sources:
        fm.open_file(path, source)
    graph = json.loads(fm.build_dependency_graph())

    def collect(nodes):
        ids = []
        for n in nodes:
            ids.append(n["id"])
            ids.extend(collect(n.get("children", [])))
        return ids

    return collect(graph.get("nodes", []))


def test_resource_block_becomes_node():
    """resource "aws_instance" "app" produces a node with id resource.aws_instance.app."""
    tf = dedent("""\
        resource "aws_instance" "app" {}
    """)
    node_ids = _nodes_flat([("/fixture/main.tf", tf)])
    assert "resource.aws_instance.app" in node_ids, f"Node not found. Got: {node_ids}"


def test_variable_block_becomes_node():
    """variable "ami_id" produces a node with id variable.ami_id."""
    tf = dedent("""\
        variable "ami_id" {
          default = "ami-12345"
        }
    """)
    node_ids = _nodes_flat([("/fixture/main.tf", tf)])
    assert "variable.ami_id" in node_ids, f"Node not found. Got: {node_ids}"


def test_module_block_becomes_node():
    """module "vpc" produces a node with id module.vpc."""
    tf = dedent("""\
        module "vpc" {
          source = "./modules/vpc"
        }
    """)
    node_ids = _nodes_flat([("/fixture/main.tf", tf)])
    assert "module.vpc" in node_ids, f"Node not found. Got: {node_ids}"


def test_output_block_becomes_node():
    """output "instance_ip" produces a node."""
    tf = dedent("""\
        output "instance_ip" {
          value = "1.2.3.4"
        }
    """)
    node_ids = _nodes_flat([("/fixture/main.tf", tf)])
    assert "output.instance_ip" in node_ids, f"Node not found. Got: {node_ids}"


def test_data_block_becomes_node():
    """data "aws_ami" "ubuntu" produces a node."""
    tf = dedent("""\
        data "aws_ami" "ubuntu" {
          most_recent = true
        }
    """)
    node_ids = _nodes_flat([("/fixture/main.tf", tf)])
    assert "data.aws_ami.ubuntu" in node_ids, f"Node not found. Got: {node_ids}"
