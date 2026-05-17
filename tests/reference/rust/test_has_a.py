from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_struct_construction():
    """let x = S {} — x has-a S (struct construction)."""
    mymod = dedent("""\
        struct S {}
        let x = S {};
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.S", "has-a")


def test_imported_struct_construction():
    """let x = C {} where C is imported from another module."""
    models = dedent("""\
        pub struct C {}
    """)
    main = dedent("""\
        use pkg::models::C;
        let x = C {};
    """)
    edges = analyze_sources([
        ("/fixture/pkg/models.rs", models),
        ("/fixture/main.rs", main),
    ])
    assert_has_edge(edges, "main.x", "pkg.models.C", "has-a")


def test_tuple_struct_construction():
    """let p = Point(1, 2) — p has-a Point (tuple struct call)."""
    mymod = dedent("""\
        struct Point(i32, i32);
        let p = Point(1, 2);
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.p", "mymod.Point", "has-a")
