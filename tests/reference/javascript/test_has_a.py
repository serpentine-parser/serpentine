from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_constructor():
    """const x = new C() — x has-a C (constructor call)."""
    mymod = dedent("""\
        class C {}
        const x = new C();
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.C", "has-a")


def test_imported_constructor():
    """const x = new C() where C is imported from another module."""
    models = dedent("""\
        export class C {}
    """)
    main = dedent("""\
        import { C } from './models';
        const x = new C();
    """)
    edges = analyze_sources([
        ("/models.ts", models),
        ("/main.ts", main),
    ])
    assert_has_edge(edges, "main.x", "models.C", "has-a")


def test_constructor_with_args():
    """const x = new C(a) — x has-a C; arg a generates a separate reference."""
    mymod = dedent("""\
        class C {}
        const a = 1;
        const x = new C(a);
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.C", "has-a")
