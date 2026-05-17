from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_name_assignment():
    """let x = y where both are defined locally."""
    mymod = dedent("""\
        let y = 1;
        let x = y;
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.y", "references")


def test_imported_name_reference():
    """x = Config {} where Config is imported — constructor or reference edge."""
    mod_a = dedent("""\
        pub struct Config {}
    """)
    main = dedent("""\
        use mod_a::Config;
        let x = Config {};
    """)
    edges = analyze_sources([
        ("/fixture/mod_a.rs", mod_a),
        ("/fixture/main.rs", main),
    ])
    # Struct construction may produce has-a or references; either is valid.
    matching = [
        e for e in edges
        if e["caller"] == "main.x" and e["callee"] == "mod_a.Config"
    ]
    assert matching, (
        f"Expected an edge main.x --> mod_a.Config (has-a or references).\n"
        f"Actual edges:\n" + "\n".join(
            f"  {e['caller']} --{e['type']}--> {e['callee']}" for e in sorted(edges, key=lambda e: e["caller"])
        )
    )


def test_local_expression():
    """let x = a + b — both operands generate references."""
    mymod = dedent("""\
        let a = 1;
        let b = 2;
        let x = a + b;
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.a", "references")
    assert_has_edge(edges, "mymod.x", "mymod.b", "references")


def test_type_annotation_param():
    """fn f(x: T) — T is referenced from f."""
    mymod = dedent("""\
        struct T {}
        fn f(x: T) {}
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.f", "mymod.T", "references")


def test_type_annotation_return():
    """fn f() -> T — T is referenced from f."""
    mymod = dedent("""\
        struct T {}
        fn f() -> T { todo!() }
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.f", "mymod.T", "references")
