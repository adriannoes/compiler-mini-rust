use compiler_mini_rust::{LexErrorKind, Position, TokenKind, lex};

fn assert_tokens(source: &[u8], expected: &[(TokenKind, &str, usize, usize)]) {
    let tokens = lex(source).unwrap();
    let actual: Vec<_> = tokens
        .iter()
        .map(|t| (t.kind, t.text.as_str(), t.position.line, t.position.column))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn first_program_includes_punctuation_and_eof() {
    use TokenKind::*;
    assert_tokens(
        include_bytes!("../examples/return_42.mc"),
        &[
            (Int, "int", 1, 1),
            (Identifier, "main", 1, 5),
            (LeftParen, "(", 1, 9),
            (RightParen, ")", 1, 10),
            (LeftBrace, "{", 1, 12),
            (Return, "return", 2, 3),
            (Integer, "42", 2, 10),
            (Semicolon, ";", 2, 12),
            (RightBrace, "}", 3, 1),
            (Eof, "", 4, 1),
        ],
    );
}

#[test]
fn keywords_require_a_complete_case_sensitive_identifier() {
    use TokenKind::*;
    assert_tokens(
        b"int2 _return Return return0 9a",
        &[
            (Identifier, "int2", 1, 1),
            (Identifier, "_return", 1, 6),
            (Identifier, "Return", 1, 14),
            (Identifier, "return0", 1, 21),
            (Integer, "9", 1, 29),
            (Identifier, "a", 1, 30),
            (Eof, "", 1, 31),
        ],
    );
}

#[test]
fn only_exact_keyword_spellings_are_reserved() {
    use TokenKind::*;
    for (word, kind) in [
        ("int", Int),
        ("bool", Bool),
        ("void", Void),
        ("if", If),
        ("else", Else),
        ("while", While),
        ("return", Return),
        ("true", True),
        ("false", False),
    ] {
        assert_tokens(
            word.as_bytes(),
            &[(kind, word, 1, 1), (Eof, "", 1, word.len() + 1)],
        );
        for name in [
            format!("{word}2"),
            format!("_{word}"),
            word.to_ascii_uppercase(),
        ] {
            assert_tokens(
                name.as_bytes(),
                &[(Identifier, &name, 1, 1), (Eof, "", 1, name.len() + 1)],
            );
        }
    }
}

#[test]
fn integer_text_survives_the_source_buffer_and_preserves_all_digits() {
    let tokens = {
        let source = String::from("0009223372036854775808000");
        lex(source.as_bytes()).unwrap()
    };
    assert_eq!(tokens[0].kind, TokenKind::Integer);
    assert_eq!(tokens[0].text, "0009223372036854775808000");
}

#[test]
fn skipped_bytes_update_positions_and_separate_tokens() {
    use TokenKind::*;
    assert_tokens(
        b"\x0b\x0c\tint/*\xff\xc3\xa9*/return\r\n//\xff\r{\n}\t",
        &[
            (Int, "int", 1, 4),
            (Return, "return", 1, 14),
            (LeftBrace, "{", 3, 1),
            (RightBrace, "}", 4, 1),
            (Eof, "", 4, 3),
        ],
    );
}

#[test]
fn empty_input_and_comments_at_eof() {
    use TokenKind::*;
    assert_tokens(b"", &[(Eof, "", 1, 1)]);
    assert_tokens(b"//", &[(Eof, "", 1, 3)]);
    assert_tokens(b"/* /* */int//", &[(Int, "int", 1, 9), (Eof, "", 1, 14)]);
}

#[test]
fn operators_choose_the_longest_supported_spelling() {
    use TokenKind::*;
    assert_tokens(
        b"=== !== <== >== &&! ||!",
        &[
            (EqualEqual, "==", 1, 1),
            (Assign, "=", 1, 3),
            (BangEqual, "!=", 1, 5),
            (Assign, "=", 1, 7),
            (LessEqual, "<=", 1, 9),
            (Assign, "=", 1, 11),
            (GreaterEqual, ">=", 1, 13),
            (Assign, "=", 1, 15),
            (AndAnd, "&&", 1, 17),
            (Bang, "!", 1, 19),
            (OrOr, "||", 1, 21),
            (Bang, "!", 1, 23),
            (Eof, "", 1, 24),
        ],
    );
}

#[test]
fn unsupported_compound_operators_split_into_individual_tokens() {
    use TokenKind::*;
    assert_tokens(
        b"-42,++--+=/=",
        &[
            (Minus, "-", 1, 1),
            (Integer, "42", 1, 2),
            (Comma, ",", 1, 4),
            (Plus, "+", 1, 5),
            (Plus, "+", 1, 6),
            (Minus, "-", 1, 7),
            (Minus, "-", 1, 8),
            (Plus, "+", 1, 9),
            (Assign, "=", 1, 10),
            (Slash, "/", 1, 11),
            (Assign, "=", 1, 12),
            (Eof, "", 1, 13),
        ],
    );
}

#[test]
fn comments_and_spaces_separate_operators() {
    use TokenKind::*;
    assert_tokens(
        b"=/**/= = = 8/**//2// line\r\n/\t/*\xff*/ /",
        &[
            (Assign, "=", 1, 1),
            (Assign, "=", 1, 6),
            (Assign, "=", 1, 8),
            (Assign, "=", 1, 10),
            (Integer, "8", 1, 12),
            (Slash, "/", 1, 17),
            (Integer, "2", 1, 18),
            (Slash, "/", 2, 1),
            (Slash, "/", 2, 9),
            (Eof, "", 2, 10),
        ],
    );
}

#[test]
fn incomplete_logical_operators_report_the_unpaired_byte() {
    for (input, byte, column) in [
        (b"&&&".as_slice(), b'&', 3),
        (b"|||", b'|', 3),
        (b"&/**/&", b'&', 1),
        (b"| |", b'|', 1),
    ] {
        let error = lex(input).unwrap_err();
        assert_eq!(error.kind, LexErrorKind::UnexpectedByte(byte));
        assert_eq!(error.position, Position { line: 1, column });
    }
}

#[test]
fn unsupported_bytes_report_the_first_position_and_exact_message() {
    for (byte, message) in [
        (b'@', "Unexpected character \"@\"."),
        (b'"', "Unexpected character \"\\\"\"."),
        (b'\\', "Unexpected character \"\\\\\"."),
        (b'&', "Unexpected character \"&\"."),
        (b'|', "Unexpected character \"|\"."),
        (0, "Unexpected byte 0x00."),
        (0xff, "Unexpected byte 0xFF."),
    ] {
        let error = lex(&[b'\n', b'\t', byte, b'@']).unwrap_err();
        assert_eq!(error.position, Position { line: 2, column: 2 });
        assert_eq!(error.kind, LexErrorKind::UnexpectedByte(byte));
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn unfinished_block_reports_its_opening_slash() {
    for source in [b"int\r\n  /*".as_slice(), b"int\n  /*/", b"int\r  /*\n*"] {
        let error = lex(source).unwrap_err();
        assert_eq!(error.position, Position { line: 2, column: 3 });
        assert_eq!(error.kind, LexErrorKind::UnterminatedBlockComment);
        assert_eq!(error.to_string(), "Unterminated block comment.");
    }
}
