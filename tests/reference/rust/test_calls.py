from textwrap import dedent

from ..helpers import analyze_sources, assert_has_edge


def test_same_module_call():
    """g calls f defined in the same module."""
    mymod = dedent("""\
        fn f() {}
        fn g() { f(); }
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.g", "mymod.f", "calls")


def test_imported_function_call():
    """main.g calls mod_a.f imported from another module."""
    mod_a = dedent("""\
        pub fn f() {}
    """)
    main = dedent("""\
        use mod_a::f;
        fn g() { f(); }
    """)
    edges = analyze_sources([
        ("/fixture/mod_a.rs", mod_a),
        ("/fixture/main.rs", main),
    ])
    assert_has_edge(edges, "main.g", "mod_a.f", "calls")


def test_method_call_on_instance():
    """s.method() where s is an instance of S."""
    mymod = dedent("""\
        struct S {}
        impl S {
            fn method(&self) {}
        }
        let s = S {};
        s.method();
    """)
    edges = analyze_sources([("/fixture/mymod.rs", mymod)])
    assert_has_edge(edges, "mymod.s", "mymod.S.method", "calls")
