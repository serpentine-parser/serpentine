from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_base_class():
    """class B extends A — B is-a A (defined in same module)."""
    mymod = dedent("""\
        class A {}
        class B extends A {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.B", "mymod.A", "is-a")


def test_imported_base_class():
    """class B extends Base where Base is imported from another module."""
    models = dedent("""\
        export class Base {}
    """)
    main = dedent("""\
        import { Base } from './models';
        class B extends Base {}
    """)
    edges = analyze_sources([
        ("/models.ts", models),
        ("/main.ts", main),
    ])
    assert_has_edge(edges, "main.B", "models.Base", "is-a")


def test_multiple_inheritance_ts():
    """class C extends A implements I — C is-a A (extends gives is-a)."""
    mymod = dedent("""\
        class A {}
        interface I {}
        class C extends A implements I {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.C", "mymod.A", "is-a")
