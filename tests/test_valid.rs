use serde_json;

use pretty_assertions::assert_eq;
use serde_json::Map as JsonMap;
use serde_json::Value as Json;

use toml_edit::{Document, Item, Iter, Value};

fn pair_to_json((key, value): (&str, Item)) -> (String, Json) {
    fn typed_json(s: &str, json: Json) -> Json {
        let mut map = JsonMap::new();
        map.insert("type".to_owned(), Json::String(s.into()));
        map.insert("value".to_owned(), json);
        Json::Object(map)
    }
    fn value_to_json(value: &Value) -> Json {
        match *value {
            Value::String(ref s) => typed_json("string", Json::String(s.value().clone())),
            Value::Integer(ref i) => typed_json("integer", Json::String(format!("{}", i.value()))),
            Value::Float(ref f) => typed_json("float", Json::String(format!("{}", f.value()))),
            Value::Boolean(ref b) => typed_json("bool", Json::String(b.raw().into())),
            Value::DateTime(ref d) => typed_json("datetime", Json::String(d.raw().into())),
            Value::Array(ref a) => {
                let json = Json::Array(a.iter().map(value_to_json).collect::<Vec<_>>());
                typed_json("array", json)
            }
            Value::InlineTable(ref t) => {
                to_json(Box::new(t.iter().map(|(k, v)| (k, Item::Value(v.clone())))))
            }
        }
    }
    let json = match value {
        Item::Value(ref v) => value_to_json(v),
        Item::ArrayOfTables(ref arr) => Json::Array(
            arr.iter()
                .map(|t| to_json(iter_to_owned(t.iter())))
                .collect::<Vec<_>>(),
        ),
        Item::Table(ref table) => to_json(iter_to_owned(table.iter())),
        Item::None => Json::Null,
        Item::DottedKeyMarker(_) => Json::Null,
    };
    (key.to_owned(), json)
}

fn iter_to_owned(iter: Iter<'_>) -> OwnedIter<'_> {
    Box::new(iter.map(|(k, v)| (k, v.clone())))
}

type OwnedIter<'s> = Box<dyn Iterator<Item = (&'s str, Item)> + 's>;

fn to_json(iter: OwnedIter<'_>) -> Json {
    Json::Object(iter.map(pair_to_json).collect())
}

fn run(json: &str, toml: &str) {
    let doc = toml.parse::<Document>();

    assert!(doc.is_ok());
    let doc = doc.unwrap();

    let json: Json = serde_json::from_str(json).unwrap();
    let toml_json = to_json(iter_to_owned(doc.iter()));
    // compare structure with jsons
    assert_eq!(json, toml_json);

    // check round-trip equality
    let toml = doc.to_string();
    let doc = toml.parse::<Document>();
    assert!(doc.is_ok());
    let doc = doc.unwrap();
    assert_eq!(doc.to_string(), toml);
}

macro_rules! t(
    ($name:ident, $json:expr, $toml:expr) => (
        #[test]
        fn $name() {
            run(include_str!($json),
                include_str!($toml));
        }
    )
);

#[test]
fn test_table_reordering() {
    let toml = r#"
[[bin]] # bin 1
[a.b.c.e]
[a]
[other.table]
[[bin]] # bin 2
[a.b.c.d]
[a.b.c]
[[bin]] # bin 3
"#;
    let expected = r#"
[[bin]] # bin 1
[[bin]] # bin 2
[[bin]] # bin 3
[a]
[a.b.c]
[a.b.c.e]
[a.b.c.d]
[other.table]
"#;
    let doc = toml.parse::<Document>();
    assert!(doc.is_ok());
    let doc = doc.unwrap();

    assert_eq!(doc.to_string(), expected);
    assert_eq!(doc.to_string_in_original_order(), toml);
}

#[test]
fn test_key_unification() {
    let toml = r#"
[a]
[a.'b'.c]
[a."b".c.e]
[a.b.c.d]
"#;
    let expected = r#"
[a]
[a.'b'.c]
[a.'b'.c.e]
[a.'b'.c.d]
"#;
    let doc = toml.parse::<Document>();
    assert!(doc.is_ok());
    let doc = doc.unwrap();

    assert_eq!(doc.to_string(), expected);
    assert_eq!(doc.to_string_in_original_order(), expected);
}

t!(
    test_array_empty,
    "fixtures/valid/array-empty.json",
    "fixtures/valid/array-empty.toml"
);
t!(
    test_array_nopspaces,
    "fixtures/valid/array-nospaces.json",
    "fixtures/valid/array-nospaces.toml"
);
t!(
    test_arrays_hetergeneous,
    "fixtures/valid/arrays-hetergeneous.json",
    "fixtures/valid/arrays-hetergeneous.toml"
);
t!(
    test_arrays,
    "fixtures/valid/arrays.json",
    "fixtures/valid/arrays.toml"
);
t!(
    test_arrays_nested,
    "fixtures/valid/arrays-nested.json",
    "fixtures/valid/arrays-nested.toml"
);
t!(
    test_bool,
    "fixtures/valid/bool.json",
    "fixtures/valid/bool.toml"
);
t!(
    test_comment_at_eof2,
    "fixtures/valid/comments-at-eof2.json",
    "fixtures/valid/comments-at-eof2.toml"
);
t!(
    test_comment_at_eof,
    "fixtures/valid/comments-at-eof.json",
    "fixtures/valid/comments-at-eof.toml"
);
t!(
    test_comments_everywhere,
    "fixtures/valid/comments-everywhere.json",
    "fixtures/valid/comments-everywhere.toml"
);
t!(
    test_datetime,
    "fixtures/valid/datetime.json",
    "fixtures/valid/datetime.toml"
);
t!(
    test_empty,
    "fixtures/valid/empty.json",
    "fixtures/valid/empty.toml"
);
t!(
    test_example,
    "fixtures/valid/example.json",
    "fixtures/valid/example.toml"
);
t!(
    test_float_exponent,
    "fixtures/valid/float-exponent.json",
    "fixtures/valid/float-exponent.toml"
);
t!(
    test_float,
    "fixtures/valid/float.json",
    "fixtures/valid/float.toml"
);
t!(
    test_float_underscore,
    "fixtures/valid/float-underscore.json",
    "fixtures/valid/float-underscore.toml"
);
t!(
    test_implicit_and_explicit_after,
    "fixtures/valid/implicit-and-explicit-after.json",
    "fixtures/valid/implicit-and-explicit-after.toml"
);
t!(
    test_implicit_and_explicit_before,
    "fixtures/valid/implicit-and-explicit-before.json",
    "fixtures/valid/implicit-and-explicit-before.toml"
);
t!(
    test_implicit_groups,
    "fixtures/valid/implicit-groups.json",
    "fixtures/valid/implicit-groups.toml"
);
t!(
    test_inline_table,
    "fixtures/valid/inline-table.json",
    "fixtures/valid/inline-table.toml"
);
t!(
    test_integer,
    "fixtures/valid/integer.json",
    "fixtures/valid/integer.toml"
);
t!(
    test_integer_underscore,
    "fixtures/valid/integer-underscore.json",
    "fixtures/valid/integer-underscore.toml"
);
t!(
    test_keys_equal_nospace,
    "fixtures/valid/key-equals-nospace.json",
    "fixtures/valid/key-equals-nospace.toml"
);
t!(
    test_key_numeric,
    "fixtures/valid/key-numeric.json",
    "fixtures/valid/key-numeric.toml"
);
t!(
    test_key_space,
    "fixtures/valid/key-space.json",
    "fixtures/valid/key-space.toml"
);
t!(
    test_key_special_chars,
    "fixtures/valid/key-special-chars.json",
    "fixtures/valid/key-special-chars.toml"
);
t!(
    test_long_float,
    "fixtures/valid/long-float.json",
    "fixtures/valid/long-float.toml"
);
t!(
    test_long_integer,
    "fixtures/valid/long-integer.json",
    "fixtures/valid/long-integer.toml"
);
t!(
    test_multiline_string,
    "fixtures/valid/multiline-string.json",
    "fixtures/valid/multiline-string.toml"
);
t!(
    test_raw_multiline_string,
    "fixtures/valid/raw-multiline-string.json",
    "fixtures/valid/raw-multiline-string.toml"
);
t!(
    test_raw_string,
    "fixtures/valid/raw-string.json",
    "fixtures/valid/raw-string.toml"
);
t!(
    test_string_empty,
    "fixtures/valid/string-empty.json",
    "fixtures/valid/string-empty.toml"
);
t!(
    test_string_escapes,
    "fixtures/valid/string-escapes.json",
    "fixtures/valid/string-escapes.toml"
);
t!(
    test_string_nl,
    "fixtures/valid/string-nl.json",
    "fixtures/valid/string-nl.toml"
);
t!(
    test_string_simple,
    "fixtures/valid/string-simple.json",
    "fixtures/valid/string-simple.toml"
);
t!(
    test_string_with_pound,
    "fixtures/valid/string-with-pound.json",
    "fixtures/valid/string-with-pound.toml"
);
t!(
    test_table_array_implicit,
    "fixtures/valid/table-array-implicit.json",
    "fixtures/valid/table-array-implicit.toml"
);
t!(
    test_table_array_many,
    "fixtures/valid/table-array-many.json",
    "fixtures/valid/table-array-many.toml"
);
t!(
    test_table_array_nest,
    "fixtures/valid/table-array-nest.json",
    "fixtures/valid/table-array-nest.toml"
);
t!(
    test_array_one,
    "fixtures/valid/table-array-one.json",
    "fixtures/valid/table-array-one.toml"
);
t!(
    test_table_empty,
    "fixtures/valid/table-empty.json",
    "fixtures/valid/table-empty.toml"
);
t!(
    test_table_no_eol,
    "fixtures/valid/table-no-eol.json",
    "fixtures/valid/table-no-eol.toml"
);
t!(
    test_table_sub_empty,
    "fixtures/valid/table-sub-empty.json",
    "fixtures/valid/table-sub-empty.toml"
);
t!(
    test_whitespace,
    "fixtures/valid/table-whitespace.json",
    "fixtures/valid/table-whitespace.toml"
);
t!(
    test_table_with_literal_string,
    "fixtures/valid/table-with-literal-string.json",
    "fixtures/valid/table-with-literal-string.toml"
);
t!(
    test_table_with_poung,
    "fixtures/valid/table-with-pound.json",
    "fixtures/valid/table-with-pound.toml"
);
t!(
    test_table_with_single_quotes,
    "fixtures/valid/table-with-single-quotes.json",
    "fixtures/valid/table-with-single-quotes.toml"
);
t!(
    test_unicode_escape,
    "fixtures/valid/unicode-escape.json",
    "fixtures/valid/unicode-escape.toml"
);
t!(
    test_unicode_literal,
    "fixtures/valid/unicode-literal.json",
    "fixtures/valid/unicode-literal.toml"
);
t!(
    test_windows_path,
    "fixtures/valid/windows-path.json",
    "fixtures/valid/windows-path.toml"
);
t!(
    test_dotted_key,
    "fixtures/valid/dotted-key.json",
    "fixtures/valid/dotted-key.toml"
);
