use std::fs;
use std::path::{Path, PathBuf};

use hoi4_parser::{Value, export_key, generate, parse};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|err| panic!("failed to read fixture {name}: {err}"))
}

#[test]
fn fixture_basic_assignment_should_parse_and_generate() {
    let source = read_fixture("basic_assignment.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(matches!(doc.root(), Value::Object(_)));
    assert!(!rendered.trim().is_empty());
}

#[test]
fn fixture_duplicate_keys_should_keep_duplicate_suffix_metadata() {
    let source = read_fixture("duplicate_keys.txt");
    let doc = parse(&source).expect("parse should succeed");
    let Value::Object(root) = doc.root() else {
        panic!("root should be object");
    };

    assert_eq!(root.entries().len(), 2);
    let second = &root.entries()[1];
    assert_eq!(export_key(second, true), "name$$1");
}

#[test]
fn fixture_nested_quoted_should_round_trip_semantically() {
    let source = read_fixture("nested_quoted.txt");
    let first = parse(&source).expect("parse should succeed");
    let rendered = generate(&first).expect("generate should succeed");
    let second = parse(&rendered).expect("parse after generate should succeed");

    assert_eq!(first.root(), second.root());
}

#[test]
fn fixture_comment_and_hash_should_keep_hash_inside_string() {
    let source = read_fixture("comment_and_hash.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");
    let reparsed = parse(&rendered).expect("reparse should succeed");

    let Value::Object(root) = reparsed.root() else {
        panic!("root should be object");
    };
    assert_eq!(root.entries().len(), 2);
    assert_eq!(root.entries()[0].key(), "name");
    assert!(matches!(root.entries()[0].value(), Value::Scalar(v) if v.contains("#")));
}

#[test]
fn fixture_operators_should_round_trip_with_operator_style() {
    let source = read_fixture("operators.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains(">="));
    assert!(rendered.contains("<"));
}

#[test]
fn fixture_bracket_and_colon_should_keep_original_symbols() {
    let source = read_fixture("bracket_and_colon.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("[From.GetID]"));
    assert!(rendered.contains("core:china"));
}

#[test]
fn fixture_scoped_duplicate_should_not_pollute_root_key_count() {
    let source = read_fixture("scoped_duplicate.txt");
    let doc = parse(&source).expect("parse should succeed");
    let Value::Object(root) = doc.root() else {
        panic!("root should be object");
    };

    assert_eq!(root.entries().len(), 2);
    assert_eq!(root.entries()[1].key(), "name");
    assert_eq!(root.entries()[1].metadata().duplicate_index, None);
}

#[test]
fn fixture_quote_adhesion_should_parse_and_keep_all_parts() {
    let source = read_fixture("quote_adhesion.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("\"Byung-il\""));
    assert!(rendered.contains("\"Byung-soo\""));
    assert!(rendered.contains("\"Byung-ok\""));
}

#[test]
fn fixture_array_block_should_round_trip_semantically() {
    let source = read_fixture("array_block.txt");
    let first = parse(&source).expect("parse should succeed");
    let rendered = generate(&first).expect("generate should succeed");
    let second = parse(&rendered).expect("reparse should succeed");

    assert_eq!(first.root(), second.root());
}
