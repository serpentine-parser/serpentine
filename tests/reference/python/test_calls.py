from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_same_module_call():
    """g calls f defined in the same module."""
    mymod = dedent("""\
        def f():
            pass

        def g():
            f()
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.g", "mymod.f", "calls")


def test_imported_function_call():
    """main.g calls mod.f imported from another module."""
    mod = dedent("""\
        def f():
            pass
    """)
    main = dedent("""\
        from mod import f

        def g():
            f()
    """)
    edges = analyze_sources([("/fixture/mod.py", mod), ("/fixture/main.py", main)])
    assert_has_edge(edges, "main.g", "mod.f", "calls")


def test_builtin_call():
    """Assignment that calls a builtin function."""
    mymod = dedent("""\
        lst = [1, 2, 3]
        x = len(lst)
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.x", "builtins.len", "calls")


def test_method_call_on_local_instance():
    """Calling a method on an instance of a locally-defined class."""
    mymod = dedent("""\
        class MyClass:
            def method(self):
                pass

        obj = MyClass()
        obj.method()
    """)
    edges = analyze_sources([("/fixture/mymod.py", mymod)])
    assert_has_edge(edges, "mymod.obj", "mymod.MyClass.method", "calls")
