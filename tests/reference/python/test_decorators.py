from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge, assert_no_edge


def test_attribute_decorator_local_obj_has_a():
    """@app.route('/') where app is a local instance → app has-a endpoint."""
    mymod = dedent("""\
        class App:
            def route(self, path):
                pass

        app = App()

        @app.route('/')
        def index():
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.app", "mymod.index", "has-a")


def test_attribute_decorator_call_form_local_obj_has_a():
    """@main.command() where main is a local instance → main has-a decorated fn."""
    mymod = dedent("""\
        class CLI:
            def command(self):
                pass

        main = CLI()

        @main.command()
        def serve():
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.main", "mymod.serve", "has-a")


def test_plain_decorator_local_references():
    """@decorator where decorator is a local function → decorated_fn references decorator."""
    mymod = dedent("""\
        def my_decorator(fn):
            return fn

        @my_decorator
        def greet():
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.greet", "mymod.my_decorator", "references")


def test_plain_decorator_call_form_local_references():
    """@decorator() call form → decorated_fn references decorator."""
    mymod = dedent("""\
        def repeat(n):
            def wrapper(fn):
                return fn
            return wrapper

        @repeat(3)
        def hello():
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.hello", "mymod.repeat", "references")


def test_imported_decorator_attribute_form_has_a():
    """@router.get('/') where router is imported → router has-a endpoint."""
    routing = dedent("""\
        class Router:
            def get(self, path):
                pass
    """)
    main = dedent("""\
        from pkg.routing import Router

        router = Router()

        @router.get('/items')
        def list_items():
            pass
    """)
    edges = analyze_sources(
        [("/fixture/pkg/routing.py", routing), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.router", "main.list_items", "has-a")


def test_stacked_decorators_each_emit_edge():
    """Stacked decorators — each fires independently."""
    mymod = dedent("""\
        def deco_a(fn):
            return fn

        def deco_b(fn):
            return fn

        @deco_a
        @deco_b
        def handler():
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.handler", "mymod.deco_a", "references")
    assert_has_edge(edges, "mymod.handler", "mymod.deco_b", "references")


def test_builtin_decorator_no_edge():
    """@staticmethod — builtin, should not produce any edge."""
    mymod = dedent("""\
        class MyClass:
            @staticmethod
            def helper():
                pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_no_edge(edges, "mymod.MyClass.helper", "staticmethod", "references")
    assert_no_edge(edges, "mymod.MyClass.helper", "staticmethod", "has-a")


def test_external_package_decorator_no_edge():
    """@pytest.mark.parametrize — external, should not produce any local edge."""
    mymod = dedent("""\
        import pytest

        @pytest.mark.parametrize('x', [1, 2])
        def test_thing(x):
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    decorator_edges = [
        e
        for e in edges
        if e["callee"].startswith("pytest") or e["caller"].startswith("pytest")
    ]
    print(decorator_edges)
    assert not decorator_edges, (
        f"Unexpected external decorator edges: {decorator_edges}"
    )


def test_class_decorator_has_a():
    """@app.route on a class → app has-a decorated class."""
    mymod = dedent("""\
        class App:
            def route(self, path):
                pass

        app = App()

        @app.route('/admin')
        class AdminView:
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.app", "mymod.AdminView", "has-a")
