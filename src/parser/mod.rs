#![allow(clippy::unneeded_field_pattern)]
#![allow(clippy::toplevel_ref_arg)]

#[macro_use]
mod macros;
mod array;
mod datetime;
mod document;
mod errors;
mod inline_table;
mod key;
mod numbers;
pub(crate) mod strings;
mod table;
mod trivia;
mod value;

pub use self::errors::TomlError;
pub(crate) use self::key::key as key_parser;
pub(crate) use self::value::value as value_parser;

use crate::document::Document;
// use crate::decor::InternalString;
use crate::key::SimpleKey;

pub struct TomlParser {
    document: Box<Document>,
    current_table_path: Vec<SimpleKey>,
    current_table_position: usize,
}

impl Default for TomlParser {
    fn default() -> Self {
        Self {
            document: Box::new(Document::new()),
            current_table_path: Vec::new(),
            current_table_position: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::*;
    use combine::stream::state::State;
    use combine::*;
    use pretty_assertions::assert_eq;
    use std;
    use std::fmt;
    // Copied from https://github.com/colin-kiegel/rust-pretty-assertions/issues/24
    /// Wrapper around string slice that makes debug output `{:?}` to print string same way as `{}`.
    /// Used in different `assert*!` macros in combination with `pretty_assertions` crate to make
    /// test failures to show nice diffs.
    #[derive(PartialEq, Eq)]
    struct PrettyString<'a>(pub &'a str);
    /// Make diff to display string as multi-line string
    impl<'a> fmt::Debug for PrettyString<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    macro_rules! parsed_eq {
        ($parsed:ident, $expected:expr) => {{
            assert!($parsed.is_ok());
            let (v, rest) = $parsed.unwrap();
            assert_eq!(v, $expected);
            assert!(rest.input.is_empty());
        }};
    }

    macro_rules! parsed_float_eq {
        ($input:ident, $expected:expr) => {{
            let parsed = numbers::float().easy_parse(State::new($input));
            assert!(parsed.is_ok());
            let (v, rest) = parsed.unwrap();
            assert!(($expected - v).abs() < std::f64::EPSILON);
            assert!(rest.input.is_empty());
        }};
    }

    macro_rules! parsed_value_eq {
        ($input:expr) => {
            let parsed = value::value().easy_parse(State::new(*$input));
            assert!(parsed.is_ok());
            let (v, rest) = parsed.unwrap();
            assert_eq!(v.to_string(), *$input);
            assert!(rest.input.is_empty());
        };
    }

    macro_rules! parsed_date_time_eq {
        ($input:expr, $is:ident) => {{
            let parsed = value::value().easy_parse(State::new(*$input));
            assert!(parsed.is_ok());
            let (v, rest) = parsed.unwrap();
            assert_eq!(v.to_string(), *$input);
            assert!(rest.input.is_empty());
            assert!(v.is_date_time());
            assert!(v.as_date_time().unwrap().$is());
        }};
    }

    #[test]
    fn integers() {
        let cases = [
            ("+99", 99),
            ("42", 42),
            ("0", 0),
            ("-17", -17),
            ("1_000", 1_000),
            ("5_349_221", 5_349_221),
            ("1_2_3_4_5", 1_2_3_4_5),
            ("0xF", 15),
            ("0o0_755", 493),
            ("0b1_0_1", 5),
            (&std::i64::MIN.to_string()[..], std::i64::MIN),
            (&std::i64::MAX.to_string()[..], std::i64::MAX),
        ];
        for &(input, expected) in &cases {
            let parsed = numbers::integer().easy_parse(State::new(input));
            parsed_eq!(parsed, expected);
        }

        let overflow = "1000000000000000000000000000000000";
        let parsed = numbers::integer().easy_parse(State::new(overflow));
        assert!(parsed.is_err());
    }

    #[test]
    fn floats() {
        let cases = [
            ("+1.0", 1.0),
            ("3.1419", 3.1419),
            ("-0.01", -0.01),
            ("5e+22", 5e+22),
            ("1e6", 1e6),
            ("-2E-2", -2E-2),
            ("6.626e-34", 6.626e-34),
            ("9_224_617.445_991_228_313", 9_224_617.445_991_228_313),
            ("-1.7976931348623157e+308", std::f64::MIN),
            ("1.7976931348623157e+308", std::f64::MAX),
            // ("1e+400", std::f64::INFINITY),
        ];
        for &(input, expected) in &cases {
            parsed_float_eq!(input, expected);
        }
    }

    #[test]
    fn basic_string() {
        let input =
            r#""I'm a string. \"You can quote me\". Name\tJos\u00E9\nLocation\tSF. \U0002070E""#;
        let parsed = strings::string().easy_parse(State::new(input));
        parsed_eq!(
            parsed,
            "I\'m a string. \"You can quote me\". Name\tJosé\nLocation\tSF. \u{2070E}"
        )
    }

    #[test]
    fn ml_basic_string() {
        let cases = [
            (
                r#""""
Roses are red
Violets are blue""""#,
                r#"Roses are red
Violets are blue"#,
            ),
            (r#"""" \""" """"#, " \"\"\" "),
            (r#"""" \\""""#, " \\"),
        ];

        for &(input, expected) in &cases {
            let parsed = strings::string().easy_parse(State::new(input));
            parsed_eq!(parsed, expected);
        }

        let invalid_cases = [r#""""  """#, r#""""  \""""#];

        for input in &invalid_cases {
            let parsed = strings::ml_basic_string().easy_parse(State::new(*input));
            assert!(parsed.is_err());
        }
    }

    #[test]
    fn ml_basic_string_escape_ws() {
        let inputs = [
            r#""""
The quick brown \


  fox jumps over \
    the lazy dog.""""#,
            r#""""\
       The quick brown \
       fox jumps over \
       the lazy dog.\
       """"#,
        ];
        for input in &inputs {
            let parsed = strings::string().easy_parse(State::new(*input));
            parsed_eq!(parsed, "The quick brown fox jumps over the lazy dog.");
        }
        let empties = [
            r#""""\
       """"#,
            r#""""
\
  \
""""#,
        ];
        for empty in &empties {
            let parsed = strings::string().easy_parse(State::new(*empty));
            parsed_eq!(parsed, "");
        }
    }

    #[test]
    fn literal_string() {
        let inputs = [
            r#"'C:\Users\nodejs\templates'"#,
            r#"'\\ServerX\admin$\system32\'"#,
            r#"'Tom "Dubs" Preston-Werner'"#,
            r#"'<\i\c*\s*>'"#,
        ];

        for input in &inputs {
            let parsed = strings::string().easy_parse(State::new(*input));
            parsed_eq!(parsed, &input[1..input.len() - 1]);
        }
    }

    #[test]
    fn ml_literal_string() {
        let input = r#"'''I [dw]on't need \d{2} apples'''"#;
        let parsed = strings::string().easy_parse(State::new(input));
        parsed_eq!(parsed, &input[3..input.len() - 3]);
        let input = r#"'''
The first newline is
trimmed in raw strings.
   All other whitespace
   is preserved.
'''"#;
        let parsed = strings::string().easy_parse(State::new(input));
        parsed_eq!(parsed, &input[4..input.len() - 3]);
    }

    #[test]
    fn offset_date_time() {
        let inputs = [
            "1979-05-27T07:32:00Z",
            "1979-05-27T00:32:00-07:00",
            "1979-05-27T00:32:00.999999-07:00",
        ];
        for input in &inputs {
            parsed_date_time_eq!(input, is_offset_date_time);
        }
    }

    #[test]
    fn local_date_time() {
        let inputs = ["1979-05-27T07:32:00", "1979-05-27T00:32:00.999999"];
        for input in &inputs {
            parsed_date_time_eq!(input, is_local_date_time);
        }
    }

    #[test]
    fn local_date() {
        let inputs = ["1979-05-27", "2017-07-20"];
        for input in &inputs {
            parsed_date_time_eq!(input, is_local_date);
        }
    }

    #[test]
    fn local_time() {
        let inputs = ["07:32:00", "00:32:00.999999"];
        for input in &inputs {
            parsed_date_time_eq!(input, is_local_time);
        }
    }

    #[test]
    fn trivia() {
        let inputs = [
            "",
            r#" "#,
            r#"
"#,
            r#"
# comment

# comment2


"#,
            r#"
        "#,
            r#"# comment
# comment2


   "#,
        ];
        for input in &inputs {
            let parsed = trivia::ws_comment_newline().easy_parse(State::new(*input));
            assert!(parsed.is_ok());
            let (t, rest) = parsed.unwrap();
            assert!(rest.input.is_empty());
            assert_eq!(&t, input);
        }
    }

    #[test]
    fn arrays() {
        let inputs = [
            r#"[]"#,
            r#"[   ]"#,
            r#"[
  1, 2, 3
]"#,
            r#"[
  1,
  2, # this is ok
]"#,
            r#"[# comment
# comment2


   ]"#,
            r#"[# comment
# comment2
      1

#sd
,
# comment3

   ]"#,
            r#"[1]"#,
            r#"[1,]"#,
            r#"[ "all", 'strings', """are the same""", '''type''']"#,
            r#"[ 100, -2,]"#,
            r#"[1, 2, 3]"#,
            r#"[1.1, 2.1, 3.1]"#,
            r#"["a", "b", "c"]"#,
            r#"[ [ 1, 2 ], [3, 4, 5] ]"#,
            r#"[ [ 1, 2 ], ["a", "b", "c"] ]"#,
            r#"[ { x = 1, a = "2" }, {a = "a",b = "b",     c =    "c"} ]"#,
        ];
        for input in &inputs {
            parsed_value_eq!(input);
        }

        let invalid_inputs = [r#"["#, r#"[,]"#, r#"[,2]"#, r#"[1e165,,]"#, r#"[ 1, 2.0 ]"#];
        for input in &invalid_inputs {
            let parsed = array::array().easy_parse(State::new(*input));
            assert!(parsed.is_err());
        }
    }

    #[test]
    fn inline_tables() {
        let inputs = [
            r#"{}"#,
            r#"{   }"#,
            r#"{a = 1e165}"#,
            r#"{ hello = "world", a = 1}"#,
        ];
        for input in &inputs {
            parsed_value_eq!(input);
        }
        let invalid_inputs = [r#"{a = 1e165"#, r#"{ hello = "world", a = 2, hello = 1}"#];
        for input in &invalid_inputs {
            let parsed = inline_table::inline_table().easy_parse(State::new(*input));
            assert!(parsed.is_err());
        }
    }

    #[test]
    fn keys() {
        let cases = [
            ("a", "a"),
            (r#""hello\n ""#, "hello\n "),
            (r#"'hello\n '"#, "hello\\n "),
            ("a.b.c", "a.b.c"),
            ("a. b.c", "a.b.c"),
            // (r#"a.'"b"'.c"#, r#"a.'"b"'.c"#),
        ];

        for &(input, expected) in &cases {
            let parsed = key::key_path2().easy_parse(State::new(input));
            assert!(parsed.is_ok());
            let (k, rest) = parsed.unwrap();
            assert_eq!(k.get(), expected);
            assert_eq!(rest.input.len(), 0);
        }
    }

    #[test]
    fn values() {
        let inputs = [
            "1979-05-27T00:32:00.999999",
            "-239",
            "1e200",
            "9_224_617.445_991_228_313",
            r#"'''I [dw]on't need \d{2} apples'''"#,
            r#"'''
The first newline is
trimmed in raw strings.
   All other whitespace
   is preserved.
'''"#,
            r#""Jos\u00E9\n""#,
            r#""\\\"\b\/\f\n\r\t\u00E9\U000A0000""#,
            r#"{ hello = "world", a = 1}"#,
            r#"[ { x = 1, a = "2" }, {a = "a",b = "b",     c =    "c"} ]"#,
        ];
        for input in &inputs {
            parsed_value_eq!(input);
        }
    }

    #[test]
    fn documents() {
        let documents = [
            r#"
# This is a TOML document.

title = "TOML Example"

    [owner]
    name = "Tom Preston-Werner"
    dob = 1979-05-27T07:32:00-08:00 # First class dates

    [database]
    server = "192.168.1.1"
    ports = [ 8001, 8001, 8002 ]
    connection_max = 5000
    enabled = true

    [servers]

    # Indentation (tabs and/or spaces) is allowed but not required
[servers.alpha]
    ip = "10.0.0.1"
    dc = "eqdc10"

    [servers.beta]
    ip = "10.0.0.2"
    dc = "eqdc10"

    [clients]
    data = [ ["gamma", "delta"], [1, 2] ]

    # Line breaks are OK when inside arrays
hosts = [
    "alpha",
    "omega"
]

   'some.wierd .stuff'   =  """
                         like
                         that
                      #   """ # this broke my sintax highlighting
   " also. like " = '''
that
'''
   double = 2e39 # this number looks familiar
# trailing comment"#,
            r#""#,
            r#"  "#,
            r#" hello = 'darkness' # my old friend
"#,
        ];
        for document in &documents {
            let doc = TomlParser::parse(document);
            if let Err(e) = doc.clone() {
                println!("{}", e);
            }
            assert!(doc.is_ok());
            let doc = doc.unwrap();

            assert_eq!(PrettyString(document), PrettyString(&doc.to_string()));
        }

        let invalid_inputs = [r#" hello = 'darkness' # my old friend
$"#];
        for document in &invalid_inputs {
            let doc = TomlParser::parse(document);

            assert!(doc.is_err());
        }
    }
}
