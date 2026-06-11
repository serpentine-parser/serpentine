from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_plain_decorator_local_references():
    """@dec class Foo {} where dec is a local function — Foo references dec."""
    mymod = dedent("""\
        function dec(target) {}
        @dec
        class Foo {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.Foo", "mymod.dec", "references")


def test_plain_decorator_call_form():
    """@dec({}) class Foo {} — Foo references dec (call form)."""
    mymod = dedent("""\
        function dec(opts) {}
        @dec({})
        class Foo {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.Foo", "mymod.dec", "references")


def test_attribute_decorator_has_a():
    """@app.route('/') class Handler {} — app has-a Handler."""
    mymod = dedent("""\
        class App { route() {} }
        const app = new App();
        @app.route('/')
        class Handler {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.app", "mymod.Handler", "has-a")


def test_stacked_decorators():
    """@decA @decB class Foo {} — two references edges from Foo."""
    mymod = dedent("""\
        function decA(target) {}
        function decB(target) {}
        @decA
        @decB
        class Foo {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    assert_has_edge(edges, "mymod.Foo", "mymod.decA", "references")
    assert_has_edge(edges, "mymod.Foo", "mymod.decB", "references")


def test_external_decorator_no_edge():
    """@Injectable() class S {} where Injectable is from 'inversify' — no local edge."""
    mymod = dedent("""\
        import { Injectable } from 'inversify';
        @Injectable()
        class S {}
    """)
    edges = analyze_sources([("/mymod.ts", mymod)])
    decorator_edges = [
        e
        for e in edges
        if "Injectable" in e.get("callee", "") or "Injectable" in e.get("caller", "")
    ]
    print(decorator_edges)
    assert not decorator_edges, (
        f"Unexpected external decorator edges: {decorator_edges}"
    )
