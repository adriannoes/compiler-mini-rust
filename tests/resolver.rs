use compiler_mini_rust::resolver::{SymbolKind, UseKind};
use compiler_mini_rust::{Position, lex, parse, resolve_names};

#[test]
fn shadowed_names_bind_in_declaration_order() {
    let resolution = {
        let ast =
            parse(&lex(b"int f(int x) { { x; int x = x; x = x; } return x; }").unwrap()).unwrap();
        resolve_names(&ast).unwrap()
    };
    let uses = resolution.bindings();
    assert_eq!(uses.len(), 5);
    assert_eq!(uses[1].symbol, uses[2].symbol);
    assert_eq!(uses[1].symbol, uses[3].symbol);
    assert_eq!(uses[0].symbol, uses[4].symbol);
    assert_ne!(uses[0].symbol, uses[1].symbol);
    assert_eq!(uses[2].kind, UseKind::Write);
    assert_eq!(
        resolution.symbol(uses[0].symbol).unwrap().kind,
        SymbolKind::Parameter
    );
    assert_eq!(
        resolution.binding_at(uses[3].position).unwrap().kind,
        UseKind::Read
    );
}

#[test]
fn resolution_prioritizes_declarations_then_uses_in_source_order() {
    for (source, column, expected) in [
        (
            "int f(){missing;}\nint f(){}",
            5,
            "Name \"f\" is already declared in this scope.",
        ),
        (
            "int f(int x){\n int x;}",
            6,
            "Name \"x\" is already declared in this scope.",
        ),
        ("int f(){ {int x;}\n x;}", 2, "Undeclared name \"x\"."),
        ("int f(){\n x=y; int x;}", 2, "Undeclared name \"x\"."),
        ("int f(){\n g(x);}", 2, "Undeclared name \"g\"."),
        (
            "int f(){int f;\n f();}",
            2,
            "Name \"f\" does not refer to a function.",
        ),
        (
            "int f(){\n f=1;}",
            2,
            "Name \"f\" does not refer to a variable or parameter.",
        ),
        (
            "int f(){return 1;\n missing;}",
            2,
            "Undeclared name \"missing\".",
        ),
    ] {
        let ast = parse(&lex(source.as_bytes()).unwrap()).unwrap();
        let err = resolve_names(&ast).unwrap_err();
        assert_eq!(err.position, Position { line: 2, column }, "{source}");
        assert_eq!(err.message, expected, "{source}");
    }
}

#[test]
fn resolution_accepts_names_without_checking_types_or_arity() {
    let ast = parse(&lex(b"void f(void x){ bool y = 1; if (42) f(y, x); }").unwrap()).unwrap();
    assert_eq!(resolve_names(&ast).unwrap().bindings().len(), 3);
}

#[test]
fn empty_program_has_only_the_global_scope() {
    let ast = parse(&lex(b"").unwrap()).unwrap();
    assert_eq!(
        resolve_names(&ast).unwrap().to_string(),
        "Resolution\n  Scope #0 GLOBAL @1:1\n"
    );
}
