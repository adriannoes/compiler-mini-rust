use compiler_mini_rust::{compile_bytecode, lex, parse};

#[test]
fn bytecode_owns_names_positions_and_literal_digits_after_parsing() {
    let bytecode = {
        let tree = parse(&lex(b"int helper(int n) { return -(0009223372036854775808); }").unwrap())
            .unwrap();
        compile_bytecode(&tree).unwrap()
    };
    let listing = bytecode.to_string();
    assert!(listing.contains("function 0 \"helper\" int parameters=1 @1:5\n"));
    assert!(listing.contains("local 0 \"n\" int @1:16\n"));
    assert!(listing.contains("constant 0 int \"-0009223372036854775808\"\n"));
    assert_eq!(bytecode.to_string(), listing);
}

#[test]
fn compilation_checks_all_sources_before_emitting_any_code() {
    let empty = parse(&lex(b"").unwrap()).unwrap();
    assert_eq!(compile_bytecode(&empty).unwrap().to_string(), "bytecode\n");
    let tree = parse(&lex(b"int f() { return 1 / 0; } bool g() { return 42; }").unwrap()).unwrap();
    let err = compile_bytecode(&tree).unwrap_err();
    assert_eq!(err.to_string(), "Expected type bool. Received int.");
    assert_eq!(err.position.column, 38);
}
