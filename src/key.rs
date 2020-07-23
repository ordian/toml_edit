use crate::decor::InternalString;
use crate::parser;
use combine::stream::state::State;
use std::str::FromStr;
use crate::decor::Decor;

/// Key as part of a Key/Value Pair or a table header.
///
/// # Examples
///
/// ```notrust
/// [dependencies."nom"]
/// version = "5.0"
/// 'literal key' = "nonsense"
/// "basic string key" = 42
/// ```
///
/// There are 3 types of keys:
///
/// 1. Bare keys (`version` and `dependencies`)
///
/// 2. Basic quoted keys (`"basic string key"` and `"nom"`)
///
/// 3. Literal quoted keys (`'literal key'`)
///
/// For details see [toml spec](https://github.com/toml-lang/toml/#keyvalue-pair).
///
/// To parse a key use `FromStr` trait implementation: `"string".parse::<Key>()`.
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone)]
pub struct SimpleKey {
    // Repr.raw_value have things like single quotes.
    pub(crate) decor: Decor,
    pub(crate) raw: InternalString,
    key: InternalString,
}

// Generally, a key is made up of a list of simple-key's separated by dots.
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone)]
pub struct Key {
    raw: InternalString,
    pub(crate) parts: Vec<SimpleKey>,
}

// impl PartialEq for Key {
//     fn eq(&self, other: &Self) -> bool {
//         if self.key.len() != other.key.len() {
//             return false;
//         }
        
//         for i in 0..self.key.len() {
//             if self.key[i].raw_value != other.key.raw_value {
//                 return false;
//             }
//         }
//         true
//     }
// }
// impl Eq for Key {}

impl FromStr for Key {
    type Err = parser::TomlError;

    /// Tries to parse a key from a &str,
    /// if fails, tries as basic quoted key (surrounds with "")
    /// and then literal quoted key (surrounds with '')
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let basic = format!("\"{}\"", s);
        let literal = format!("'{}'", s);
        Key::try_parse(s)
            .or_else(|_| Key::try_parse(&basic))
            .or_else(|_| Key::try_parse(&literal))
    }
}

impl Key {
    fn try_parse(s: &str) -> Result<Key, parser::TomlError> {
        use combine::Parser;
        let result = parser::key_parser().easy_parse(State::new(s));
        match result {
            Ok((_, ref rest)) if !rest.input.is_empty() => {
                Err(parser::TomlError::from_unparsed(rest.positioner, s))
            }
            Ok(((raw, parts), _)) => Ok(Key::new(raw.into(), parts)),
            Err(e) => Err(parser::TomlError::new(e, s)),
        }
    }

    pub(crate) fn new(raw: InternalString, parts: Vec<SimpleKey>) -> Self {
        Self {
            raw,
            parts,
        }
    }

    /// Returns the parsed key value.
    pub fn get(&self) -> InternalString {
        let keys_parts: Vec<_> = self.parts.iter().map(|k| k.key.clone()).collect();
        keys_parts.join(".")
    }

    /// Returns the parsed key value with decorators.
    pub fn get_with_decor(&self) -> InternalString {
        let keys_parts: Vec<_> = self.parts.iter().map(|k| k.raw.clone()).collect();
        keys_parts.join(".")

        // same as raw()?
    }

    /// Returns the key raw representation.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Get key path.
    pub fn get_key_path(&self) -> &[SimpleKey] {
        &self.parts
    }

    /// Get key path.
    pub fn get_string_path(&self) -> Vec<InternalString> {
        self.parts.iter().map(|r| r.key.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_dotted_key(&self) -> bool {
        self.parts.len() > 1
    }
}


impl SimpleKey {
    // TODO: repr and raw are same?
    pub(crate) fn new(decor: Decor, raw: InternalString, key: InternalString) -> Self {
        Self {
            decor,
            raw,
            key,
        }
    }

    /// Returns the parsed key value.
    pub fn get(&self) -> &str {
        &self.key
    }

    /// Returns the key raw representation.
    pub fn raw(&self) -> &str {
        &self.raw
    }
}


#[doc(hidden)]
impl Into<InternalString> for Key {
    fn into(self) -> InternalString {
        self.get()
    }
}
