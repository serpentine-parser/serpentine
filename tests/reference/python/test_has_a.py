from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_class_constructor():
    """x = C() where C is defined in the same module."""
    mymod = dedent("""\
        class C:
            pass

        x = C()
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.C", "has-a")


def test_imported_class_constructor():
    """x = C() where C is imported from another module."""
    models = dedent("""\
        class C:
            pass
    """)
    main = dedent("""\
        from pkg.models import C

        x = C()
    """)
    edges = analyze_sources(
        [("/fixture/pkg/models.py", models), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.x", "pkg.models.C", "has-a")


def test_constructor_with_args():
    """x = C(a, b) — args don't prevent the has-a edge."""
    mymod = dedent("""\
        class C:
            pass

        a = 1
        b = 2
        x = C(a, b)
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.C", "has-a")


def test_with_statement_has_a():
    """with Foo() as x: — x has-a Foo."""
    src = dedent("""\
        class Foo:
            def __enter__(self): return self
            def __exit__(self, *a): pass
        def bar():
            with Foo() as x:
                pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", src)])
    assert_has_edge(edges, "mymod.bar.x", "mymod.Foo", "has-a")
