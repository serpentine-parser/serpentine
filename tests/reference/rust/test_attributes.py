from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge, assert_no_edge


def test_plain_attribute_local_references():
    """#[my_attr] fn f() {} — f references my_attr (local definition)."""
    mymod = dedent("""\
        fn my_attr() {}
        #[my_attr]
        fn f() {}
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.f", "mymod.my_attr", "references")


def test_imported_attribute_references():
    """#[instrument] fn f() {} where instrument is imported from macros."""
    macros = dedent("""\
        pub fn instrument() {}
    """)
    main = dedent("""\
        use macros::instrument;
        #[instrument]
        fn f() {}
    """)
    edges = analyze_sources([
        ("/fixture/macros.rs", macros),
        ("/fixture/main.rs", main),
    ])
    assert_has_edge(edges, "main.f", "macros.instrument", "references")


def test_builtin_attribute_no_edge():
    """#[inline] fn f() {} — inline is a built-in; no edge produced."""
    mymod = dedent("""\
        #[inline]
        fn f() {}
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_no_edge(edges, "mymod.f", "inline", "references")
    assert_no_edge(edges, "mymod.f", "inline", "has-a")


def test_derive_no_edge():
    """#[derive(Debug)] struct S {} — derive and Debug are built-ins; no edges."""
    mymod = dedent("""\
        #[derive(Debug)]
        struct S {}
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_no_edge(edges, "mymod.S", "derive", "references")
    assert_no_edge(edges, "mymod.S", "Debug", "references")
