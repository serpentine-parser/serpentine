from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_local_base_class():
    """class B(A) where A is defined in the same module."""
    mymod = dedent("""\
        class A:
            pass

        class B(A):
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.B", "mymod.A", "is-a")


def test_imported_base_class():
    """class B(Base) where Base is imported from another module."""
    base = dedent("""\
        class Base:
            pass
    """)
    main = dedent("""\
        from pkg.base import Base

        class B(Base):
            pass
    """)
    edges = analyze_sources(
        [("/fixture/pkg/base.py", base), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.B", "pkg.base.Base", "is-a")


def test_multiple_inheritance():
    """class C(A, B) — one is-a edge per base."""
    mymod = dedent("""\
        class A:
            pass

        class B:
            pass

        class C(A, B):
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.C", "mymod.A", "is-a")
    assert_has_edge(edges, "mymod.C", "mymod.B", "is-a")


def test_transitive_inheritance():
    """A ← B ← C: two direct is-a edges, no synthetic transitive edge."""
    mymod = dedent("""\
        class A:
            pass

        class B(A):
            pass

        class C(B):
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.B", "mymod.A", "is-a")
    assert_has_edge(edges, "mymod.C", "mymod.B", "is-a")
