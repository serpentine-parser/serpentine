from textwrap import dedent

from conftest import analyze_sources, assert_has_edge


def test_local_name_assignment():
    """x = y where both are defined locally."""
    mymod = dedent("""\
        y = 1
        x = y
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.y", "references")


def test_imported_name_reference():  # post-fix
    """x = Config where Config is imported — no call, just a name reference."""
    models = dedent("""\
        class Config:
            pass
    """)
    main = dedent("""\
        from pkg.models import Config

        x = Config
    """)
    edges = analyze_sources(
        [("/fixture/pkg/models.py", models), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.x", "pkg.models.Config", "references")


def test_imported_constant_reference():  # post-fix
    """x = CONST where CONST is an imported constant."""
    constants = dedent("""\
        CONST = 42
    """)
    main = dedent("""\
        from pkg.constants import CONST

        x = CONST
    """)
    edges = analyze_sources(
        [("/fixture/pkg/constants.py", constants), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.x", "pkg.constants.CONST", "references")


def test_local_expression():
    """x = a + b — both operands generate references."""
    mymod = dedent("""\
        a = 1
        b = 2
        x = a + b
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.a", "references")
    assert_has_edge(edges, "mymod.x", "mymod.b", "references")


def test_imported_name_in_expression():  # post-fix
    """x = LIMIT + 1 where LIMIT is an imported constant."""
    constants = dedent("""\
        LIMIT = 100
    """)
    main = dedent("""\
        from pkg.constants import LIMIT

        x = LIMIT + 1
    """)
    edges = analyze_sources(
        [("/fixture/pkg/constants.py", constants), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.x", "pkg.constants.LIMIT", "references")


def test_attribute_access():  # post-fix
    """x = obj.attr — obj is referenced."""
    mymod = dedent("""\
        class Obj:
            attr = 0

        obj = Obj()
        x = obj.attr
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.obj", "references")


def test_local_collection_references():
    """x = [a, b] — both list elements generate references."""
    mymod = dedent("""\
        a = 1
        b = 2
        x = [a, b]
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.a", "references")
    assert_has_edge(edges, "mymod.x", "mymod.b", "references")


def test_imported_collection_references():  # post-fix
    """x = [A, B] where A and B are imported."""
    items = dedent("""\
        class A:
            pass

        class B:
            pass
    """)
    main = dedent("""\
        from pkg.items import A, B

        x = [A, B]
    """)
    edges = analyze_sources(
        [("/fixture/pkg/items.py", items), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.x", "pkg.items.A", "references")
    assert_has_edge(edges, "main.x", "pkg.items.B", "references")


def test_augmented_assignment():
    """x += y — y is referenced."""
    mymod = dedent("""\
        x = 0
        y = 1
        x += y
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "mymod.y", "references")


def test_type_annotation_param():
    """def f(x: T) — T is referenced from f via UseName."""
    mymod = dedent("""\
        class T:
            pass

        def f(x: T):
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.f", "mymod.T", "references")


def test_type_annotation_return():
    """def f() -> T — T is referenced from f."""
    mymod = dedent("""\
        class T:
            pass

        def f() -> T:
            pass
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.f", "mymod.T", "references")


def test_imported_type_annotation():
    """def f(x: T) where T is imported."""
    types = dedent("""\
        class T:
            pass
    """)
    main = dedent("""\
        from pkg.types import T

        def f(x: T):
            pass
    """)
    edges = analyze_sources(
        [("/fixture/pkg/types.py", types), ("/fixture/main.py", main)]
    )
    assert_has_edge(edges, "main.f", "pkg.types.T", "references")
