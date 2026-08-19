from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_trait_impl():
    """impl MyTrait for S — S is-a MyTrait (defined in same module)."""
    mymod = dedent("""\
        trait MyTrait {}
        struct S {}
        impl MyTrait for S {}
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.S", "mymod.MyTrait", "is-a")


def test_imported_trait_impl():
    """impl MyTrait for S where MyTrait is imported from another module."""
    traits = dedent("""\
        pub trait MyTrait {}
    """)
    main = dedent("""\
        use pkg::traits::MyTrait;
        struct S {}
        impl MyTrait for S {}
    """)
    edges = analyze_sources(
        [
            ("/fixture/pkg/traits.rs", traits),
            ("/fixture/main.rs", main),
        ]
    )
    assert_has_edge(edges, "main.S", "pkg.traits.MyTrait", "is-a")
