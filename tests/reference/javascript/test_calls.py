from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_same_module_call():
    """g calls f defined in the same module."""
    mymod = dedent("""\
        function f() {}
        function g() { f(); }
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.g", "mymod.f", "calls")


def test_imported_function_call():
    """main.g calls models.f imported from another module."""
    models = dedent("""\
        export function f() {}
    """)
    main = dedent("""\
        import { f } from './models';
        function g() { f(); }
    """)
    edges = analyze_sources([
        ("/models.ts", models),
        ("/main.ts", main),
    ])
    assert_has_edge(edges, "main.g", "models.f", "calls")


def test_method_call_on_instance():
    """c.m() where c is an instance of C."""
    mymod = dedent("""\
        class C { m() {} }
        const c = new C();
        c.m();
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.c", "mymod.C.m", "calls")
