from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_name_assignment():
    """const x = y where both are defined locally."""
    mymod = dedent("""\
        const y = 1;
        const x = y;
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.y", "references")


def test_imported_name_reference():
    """const x = Config where Config is imported from another module."""
    models = dedent("""\
        export class Config {}
    """)
    main = dedent("""\
        import { Config } from './models';
        const x = Config;
    """)
    edges = analyze_sources([
        ("/models.ts", models),
        ("/main.ts", main),
    ])
    assert_has_edge(edges, "main.x", "models.Config", "references")


def test_local_expression():
    """const x = a + b — both operands generate references."""
    mymod = dedent("""\
        const a = 1;
        const b = 2;
        const x = a + b;
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.a", "references")
    assert_has_edge(edges, "mymod.x", "mymod.b", "references")


def test_type_annotation_param():
    """function f(x: T) — T is referenced from f (TypeScript)."""
    mymod = dedent("""\
        class T {}
        function f(x: T) {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.f", "mymod.T", "references")
