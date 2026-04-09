use std::fs;
use std::path::{Path, PathBuf};

use hoi4_parser::{export_key, generate, parse, Value};

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

#[test]
fn fixture_rgb_block_scalar_should_parse_and_round_trip() {
    let source = read_fixture("rgb_block_scalar.txt");
    let first = parse(&source).expect("parse should succeed");
    let rendered = generate(&first).expect("generate should succeed");
    let second = parse(&rendered).expect("reparse should succeed");

    assert_eq!(first.root(), second.root());
    assert!(rendered.contains("color = {\n"));
    assert!(rendered.contains("\trgb\n"));
    assert!(rendered.contains("\t153\n"));
}

#[test]
fn fixture_hsv_block_scalar_should_parse_and_round_trip() {
    let source = read_fixture("hsv_block_scalar.txt");
    let first = parse(&source).expect("parse should succeed");
    let rendered = generate(&first).expect("generate should succeed");
    let second = parse(&rendered).expect("reparse should succeed");

    assert_eq!(first.root(), second.root());
    assert!(rendered.contains("color = {\n"));
    assert!(rendered.contains("\tHSV\n"));
    assert!(rendered.contains("\t0.15\n"));
}

#[test]
fn fixture_hsv_lowercase_block_scalar_should_parse_and_round_trip() {
    let source = "country = {\n\tcolor = hsv { 0.1 0.47 0.8 }\n}";
    let first = parse(source).expect("parse should succeed");
    let rendered = generate(&first).expect("generate should succeed");
    let second = parse(&rendered).expect("reparse should succeed");

    assert_eq!(first.root(), second.root());
    assert!(rendered.contains("color = {\n"));
    assert!(rendered.contains("\thsv\n"));
}

#[test]
fn fixture_multiline_condition_block_should_keep_multiline_and_operator_symbols() {
    let source = read_fixture("multiline_condition_block.txt");
    let first = parse(&source).expect("parse should succeed");
    let rendered = generate(&first).expect("generate should succeed");

    assert!(rendered.contains("FROM = {\n"));
    assert!(rendered.contains("\thas_war_support > 0.1"));
    assert!(rendered.contains("\tcommand_power > 1"));
    assert!(!rendered.contains("&gt;"));
}

#[test]
fn fixture_multiline_single_condition_should_keep_block_style() {
    let source = read_fixture("multiline_single_condition_block.txt");
    let first = parse(&source).expect("parse should succeed");
    let rendered = generate(&first).expect("generate should succeed");
    let second = parse(&rendered).expect("reparse should succeed");

    assert_eq!(first.root(), second.root());
    assert!(rendered.contains("check_variable = {\n"));
    assert!(rendered.contains("\tnum_units_offensive_combats > 6"));
    assert!(!rendered.contains("check_variable = { num_units_offensive_combats > 6 }"));
}

#[test]
fn fixture_singleline_condition_should_expand_to_block_style() {
    let source = read_fixture("singleline_condition_block.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("check_variable = {\n"));
    assert!(rendered.contains("\tnum_units_offensive_combats > 6"));
}

#[test]
fn fixture_single_token_block_should_expand_to_multiline_style() {
    let source = read_fixture("single_token_block.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("change_camo_when = {\n"));
    assert!(rendered.contains("\tsnow\n"));
    assert!(rendered.contains("forbid_camo_when = {\n"));
    assert!(rendered.contains("\tdesert\n"));
}

#[test]
fn fixture_multi_token_list_block_should_expand_each_item_to_multiline() {
    let source = read_fixture("multi_token_list_block.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("type = {\n"));
    assert!(rendered.contains("\tfighter\n"));
    assert!(rendered.contains("\theavy_fighter\n"));
    assert!(rendered.contains("\tinterceptor\n"));
}

#[test]
fn fixture_array_mixed_not_object_should_not_split_not_assignment() {
    let source = read_fixture("array_mixed_not_object.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("NOT = {\n"));
    assert!(!rendered.contains("NOT =\n"));
}

#[test]
fn fixture_empty_object_block_should_render_multiline_empty_block() {
    let source = read_fixture("empty_object_block.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("blocked_for = {\n"));
    assert!(rendered.contains("available_for = {\n"));
    assert!(!rendered.contains("blocked_for = {}"));
    assert!(!rendered.contains("available_for = {}"));
}

#[test]
fn fixture_key_value_boundary_should_not_merge_neighbor_assignments() {
    let source = read_fixture("key_value_boundary.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("ruling_only = yes\n"));
    assert!(rendered.contains("character = PRC_mao_zedong\n"));
    assert!(!rendered.contains("ruling_only = yes character = PRC_mao_zedong"));
}

#[test]
fn fixture_array_following_equals_should_keep_compact_assignment_block() {
    let source = read_fixture("array_following_equals_block.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("any_of = {\n"));
    assert!(!rendered.contains("any_of =\n"));
}

#[test]
fn fixture_implicit_operator_assignment_should_not_merge_with_previous_value() {
    let source = read_fixture("implicit_operator_assignment.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("unit = battleship\n"));
    assert!(rendered.contains("size > 9\n"));
    assert!(!rendered.contains("unit = battleship size > 9"));
}

#[test]
fn fixture_quoted_scalar_with_underscore_should_drop_quotes_like_java() {
    let source = "obj = {\n\tname = \"BLITZKRIEG_NAME\"\n\tdesc = \"BLITZKRIEG_DESC\"\n\tpicture = \"GFX_select_date_1939\"\n}";
    let doc = parse(source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");
    assert!(rendered.contains("name = BLITZKRIEG_NAME"));
    assert!(rendered.contains("desc = BLITZKRIEG_DESC"));
    assert!(rendered.contains("picture = GFX_select_date_1939"));
    assert!(!rendered.contains("name = \"BLITZKRIEG_NAME\""));
}

#[test]
fn fixture_quoted_scalar_with_space_should_keep_quotes() {
    let source = "obj = {\n\thas_dlc = \"No Compromise, No Surrender\"\n}";
    let doc = parse(source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");
    assert!(rendered.contains("has_dlc = \"No Compromise, No Surrender\""));
}

#[test]
fn fixture_quoted_lowercase_identifier_with_underscore_should_keep_quotes() {
    let source = "obj = {\n\tdivision_types = { \"light_armor\" \"medium_armor\" }\n}";
    let doc = parse(source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");
    assert!(rendered.contains("\t\"light_armor\"\n"));
    assert!(rendered.contains("\t\"medium_armor\"\n"));
}

#[test]
fn fixture_identifier_list_same_line_should_expand_to_multiline_items() {
    let source = read_fixture("list_identifiers_same_line.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("preferred_countries = {\n"));
    assert!(rendered.contains("\tGER\n"));
    assert!(rendered.contains("\tSLO\n"));
    assert!(!rendered.contains("GER SLO"));
}

#[test]
fn fixture_prefix_limit_block_should_keep_compact_prefix_line() {
    let source = read_fixture("prefix_limit_block.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("faction_members\n"));
    assert!(rendered.contains("limit = {\n"));
}

#[test]
fn fixture_anonymous_object_array_should_render_anonymous_block() {
    let source = read_fixture("anonymous_object_array.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("milestones = {\n"));
    assert!(rendered.contains("\t\t{\n"));
    assert!(rendered.contains("\t\t\tcategory_fighter = {\n"));
    assert!(!rendered.contains("\t\t# = {\n"));
    assert!(!rendered.contains("\t\t{ category_fighter ="));
}

#[test]
fn fixture_prefix_chain_limit_block_should_split_prefixes_generically() {
    let source = read_fixture("prefix_chain_limit_block.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("\t\tfaction_members\n"));
    assert!(rendered.contains("\t\towned_states\n"));
    assert!(rendered.contains("\t\tlimit = {\n"));
    assert!(!rendered.contains("faction_members owned_states limit ="));
}

#[test]
fn fixture_unicode_identifier_list_should_split_to_multiline_items() {
    let source = read_fixture("unicode_identifier_list.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("\tAdler\n"));
    assert!(rendered.contains("\tDöll\n"));
    assert!(rendered.contains("\tMüller\n"));
    assert!(rendered.contains("\tZürckgiebel\n"));
    assert!(!rendered.contains("Adler Döll Müller"));
}

#[test]
fn fixture_mixed_quoted_name_list_should_split_to_multiline_items() {
    let source = read_fixture("mixed_quoted_name_list.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("\tAnthoine\n"));
    assert!(rendered.contains("\t\"Baraguey d'Hilliers\"\n"));
    assert!(rendered.contains("\tBazaine\n"));
    assert!(rendered.contains("\t\"Boué de Lapeyrère\"\n"));
}

#[test]
fn fixture_mixed_quoted_name_list_with_apostrophe_should_split_to_multiline_items() {
    let source = read_fixture("mixed_quoted_name_list_with_apostrophe.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("\t\"de MacMahon\"\n"));
    assert!(rendered.contains("\td'Orleans\n"));
    assert!(rendered.contains("\tDubois\n"));
    assert!(!rendered.contains("\"de MacMahon\" \"de Montaignac\""));
}

#[test]
fn fixture_mixed_quoted_name_list_with_commas_should_split_and_keep_commas() {
    let source = read_fixture("mixed_quoted_name_list_with_commas.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("\t\"H 1\"\n"));
    assert!(rendered.contains("\t,\n"));
    assert!(rendered.contains("\t\"H 2\"\n"));
    assert!(rendered.contains("\t\"Balilla\"\n"));
}

#[test]
fn fixture_attached_operator_identifier_should_render_spaced_single_line() {
    let source = read_fixture("attached_operator_identifier.txt");
    let doc = parse(&source).expect("parse should succeed");
    let rendered = generate(&doc).expect("generate should succeed");

    assert!(rendered.contains("num_owned_controlled_states < controlled_owned_75"));
    assert!(!rendered.contains("num_owned_controlled_states\n"));
}
