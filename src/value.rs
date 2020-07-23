use crate::decor::{Decor, Formatted, InternalString};
use crate::key::Key;
use crate::parser;
use crate::table::{Item, Iter, KeyValuePairs, TableKeyValue, TableLike};
use crate::{decorated, formatted};
use chrono::{self, FixedOffset};
use combine::stream::state::State;
use linked_hash_map::LinkedHashMap;
use std::mem;
use std::str::FromStr;

/// Representation of a TOML Value (as part of a Key/Value Pair).
#[derive(Debug, Clone)]
pub enum Value {
    /// A 64-bit integer value.
    Integer(Formatted<i64>),
    /// A string value.
    String(Formatted<String>),
    /// A 64-bit float value.
    Float(Formatted<f64>),
    /// A Date-Time value.
    DateTime(Formatted<DateTime>),
    /// A boolean value.
    Boolean(Formatted<bool>),
    /// An inline array of values.
    Array(Array),
    /// An inline table of key/value pairs.
    InlineTable(InlineTable),
}

/// Type representing a TOML Date-Time,
/// payload of the `Value::DateTime` variant's value
#[derive(Eq, PartialEq, Clone, Debug, Hash)]
pub enum DateTime {
    /// An RFC 3339 formatted date-time with offset.
    OffsetDateTime(chrono::DateTime<FixedOffset>),
    /// An RFC 3339 formatted date-time without offset.
    LocalDateTime(chrono::NaiveDateTime),
    /// Date portion of an RFC 3339 formatted date-time.
    LocalDate(chrono::NaiveDate),
    /// Time portion of an RFC 3339 formatted date-time.
    LocalTime(chrono::NaiveTime),
}

/// Type representing a TOML array,
/// payload of the `Value::Array` variant's value
#[derive(Debug, Default, Clone)]
pub struct Array {
    // always Vec<Item::Value>
    pub(crate) values: Vec<Item>,
    // `trailing` represents whitespaces, newlines
    // and comments in an empty array or after the trailing comma
    pub(crate) trailing: InternalString,
    pub(crate) trailing_comma: bool,
    // prefix before `[` and suffix after `]`
    pub(crate) decor: Decor,
}

/// Type representing a TOML inline table,
/// payload of the `Value::InlineTable` variant
#[derive(Debug, Default, Clone)]
pub struct InlineTable {
    pub(crate) items: KeyValuePairs,
    // `preamble` represents whitespaces in an empty table
    pub(crate) preamble: InternalString,
    // prefix before `{` and suffix after `}`
    pub(crate) decor: Decor,
}

#[derive(Eq, PartialEq, Clone, Copy, Debug, Hash)]
pub(crate) enum ValueType {
    None,
    Integer,
    String,
    Float,
    DateTime,
    Boolean,
    Array,
    InlineTable,
}

/// An iterator type over `Array`'s values.
pub type ArrayIter<'a> = Box<dyn Iterator<Item = &'a Value> + 'a>;

impl Array {
    /// Returns the length of the underlying Vec.
    /// To get the actual number of items use `a.iter().count()`.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Return true iff `self.len() == 0`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over all values.
    pub fn iter(&self) -> ArrayIter<'_> {
        Box::new(self.values.iter().filter_map(Item::as_value))
    }

    /// Appends a new value to the end of the array, applying default formatting to it.
    ///
    /// Returns an error if the value was of a different type than the values in the array.
    pub fn push<V: Into<Value>>(&mut self, v: V) -> Result<(), Value> {
        self.value_op(v.into(), true, |items, value| {
            items.push(Item::Value(value))
        })
    }

    /// Appends a new, already formatted value to the end of the array.
    ///
    /// Returns an error if the value was of a different type than the array.
    pub fn push_formatted(&mut self, v: Value) -> Result<(), Value> {
        self.value_op(v, false, |items, value| items.push(Item::Value(value)))
    }

    /// Inserts an element at the given position within the array, applying default formatting to
    /// it and shifting all values after it to the right.
    ///
    /// Returns an error if the value was of a different type than the values in the array.
    ///
    /// Panics if `index > len`.
    pub fn insert<V: Into<Value>>(&mut self, index: usize, v: V) -> Result<(), Value> {
        self.value_op(v.into(), true, |items, value| {
            items.insert(index, Item::Value(value))
        })
    }

    /// Inserts an already formatted value at the given position within the array, shifting all
    /// values after it to the right.
    ///
    /// Returns an error if the value was of a different type than the values in the array.
    ///
    /// Panics if `index > len`.
    pub fn insert_formatted(&mut self, index: usize, v: Value) -> Result<(), Value> {
        self.value_op(v, false, |items, value| {
            items.insert(index, Item::Value(value))
        })
    }

    /// Replaces the element at the given position within the array, preserving existing formatting.
    ///
    /// Returns an error if the replacement was of a different type than the values in the array.
    ///
    /// Panics if `index >= len`.
    pub fn replace<V: Into<Value>>(&mut self, index: usize, v: V) -> Result<Value, Value> {
        // Read the existing value's decor and preserve it.
        let existing_decor = self
            .get(index)
            .unwrap_or_else(|| panic!("index {} out of bounds (len = {})", index, self.len()))
            .decor();
        let value = decorated(v.into(), existing_decor.prefix(), existing_decor.suffix());
        self.replace_formatted(index, value)
    }

    /// Replaces the element at the given position within the array with an already formatted value.
    ///
    /// Returns an error if the replacement was of a different type than the values in the array.
    ///
    /// Panics if `index >= len`.
    pub fn replace_formatted(&mut self, index: usize, v: Value) -> Result<Value, Value> {
        self.value_op(v, false, |items, value| {
            match mem::replace(&mut items[index], Item::Value(value)) {
                Item::Value(old_value) => old_value,
                x => panic!("non-value item {:?} in an array", x),
            }
        })
    }

    /// Returns a reference to the value at the given index, or `None` if the index is out of
    /// bounds.
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index).and_then(Item::as_value)
    }

    /// Removes the value at the given index.
    pub fn remove(&mut self, index: usize) -> Value {
        let removed = self.values.remove(index);
        if self.is_empty() {
            self.trailing_comma = false;
        }
        match removed {
            Item::Value(v) => v,
            x => panic!("non-value item {:?} in an array", x),
        }
    }

    /// Auto formats the array.
    pub fn fmt(&mut self) {
        formatted::decorate_array(self);
    }

    fn value_op<T>(
        &mut self,
        v: Value,
        decorate: bool,
        op: impl FnOnce(&mut Vec<Item>, Value) -> T,
    ) -> Result<T, Value> {
        let mut value = v;
        if !self.is_empty() && decorate {
            formatted::decorate(&mut value, " ", "");
        } else if decorate {
            formatted::decorate(&mut value, "", "");
        }
        if self.is_empty() || value.get_type() == self.value_type() {
            Ok(op(&mut self.values, value))
        } else {
            Err(value)
        }
    }

    pub(crate) fn value_type(&self) -> ValueType {
        if let Some(value) = self.values.get(0).and_then(Item::as_value) {
            value.get_type()
        } else {
            ValueType::None
        }
    }
}

/// An iterator type over key/value pairs of an inline table.
pub type InlineTableIter<'a> = Box<dyn Iterator<Item = (&'a str, &'a Value)> + 'a>;

impl InlineTable {
    /// Returns the number of key/value pairs.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns true iff the table is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over key/value pairs.
    pub fn iter(&self) -> InlineTableIter<'_> {
        Box::new(
            self.items
                .iter()
                .filter(|&(_, kv)| kv.value.is_value())
                .map(|(k, kv)| (&k[..], kv.value.as_value().unwrap())),
        )
    }

    /// Sorts the key/value pairs by key.
    pub fn sort(&mut self) {
        sort_key_value_pairs(&mut self.items);
    }

    /// Returns true iff the table contains given key.
    pub fn contains_key(&self, key: &str) -> bool {
        if let Some(kv) = self.items.get(key) {
            !kv.value.is_none()
        } else {
            false
        }
    }

    /// Merges the key/value pairs into the `other` table leaving
    /// `self` empty.
    pub fn merge_into(&mut self, other: &mut InlineTable) {
        let items = mem::replace(&mut self.items, KeyValuePairs::new());
        for (k, kv) in items {
            other.items.insert(k, kv);
        }
    }

    /// Inserts a key/value pair if the table does not contain the key.
    /// Returns a mutable reference to the corresponding value.
    pub fn get_or_insert<V: Into<Value>>(&mut self, key: &str, value: V) -> &mut Value {
        let parsed = key.parse::<Key>().expect("invalid key");
        self.items
            .entry(parsed.get().to_owned())
            .or_insert(formatted::to_key_value(key, value.into()))
            .value
            .as_value_mut()
            .expect("non-value type in inline table")
    }

    /// Auto formats the table.
    pub fn fmt(&mut self) {
        formatted::decorate_inline_table(self);
    }

    /// Removes a key/value pair given the key.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.items
            .remove(key)
            .and_then(|kv| kv.value.as_value().cloned())
    }

    /// Return an optional reference to the value at the given the key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.items.get(key).and_then(|kv| kv.value.as_value())
    }

    /// Return an optional mutable reference to the value at the given the key.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.items
            .get_mut(key)
            .and_then(|kv| kv.value.as_value_mut())
    }
}

impl TableLike for InlineTable {
    fn iter(&self) -> Iter<'_> {
        Box::new(self.items.iter().map(|(key, kv)| (&key[..], &kv.value)))
    }
    fn get<'s>(&'s self, key: &str) -> Option<&'s Item> {
        self.items.get(key).map(|kv| &kv.value)
    }
}

/// Downcasting
impl DateTime {
    /// Casts `self` to offset date-time.
    pub fn as_offset_date_time(&self) -> Option<&chrono::DateTime<FixedOffset>> {
        match *self {
            DateTime::OffsetDateTime(ref dt) => Some(dt),
            _ => None,
        }
    }
    /// Casts `self` to local date-time.
    pub fn as_local_date_time(&self) -> Option<&chrono::NaiveDateTime> {
        match *self {
            DateTime::LocalDateTime(ref dt) => Some(dt),
            _ => None,
        }
    }
    /// Casts `self` to local date.
    pub fn as_local_date(&self) -> Option<&chrono::NaiveDate> {
        match *self {
            DateTime::LocalDate(ref d) => Some(d),
            _ => None,
        }
    }
    /// Casts `self` to local time.
    pub fn as_local_time(&self) -> Option<&chrono::NaiveTime> {
        match *self {
            DateTime::LocalTime(ref t) => Some(t),
            _ => None,
        }
    }
    /// Returns true iff `self` is an offset date-time.
    pub fn is_offset_date_time(&self) -> bool {
        self.as_offset_date_time().is_some()
    }
    /// Returns true iff `self` is a local date-time.
    pub fn is_local_date_time(&self) -> bool {
        self.as_local_date_time().is_some()
    }
    /// Returns true iff `self` is a local date.
    pub fn is_local_date(&self) -> bool {
        self.as_local_date().is_some()
    }
    /// Returns true iff `self` is a local time.
    pub fn is_local_time(&self) -> bool {
        self.as_local_time().is_some()
    }
}

/// Downcasting
impl Value {
    /// Casts `self` to integer.
    pub fn as_integer(&self) -> Option<i64> {
        match *self {
            Value::Integer(ref value) => Some(*value.value()),
            _ => None,
        }
    }

    /// Returns true iff `self` is an integer.
    pub fn is_integer(&self) -> bool {
        self.as_integer().is_some()
    }

    /// Casts `self` to float.
    pub fn as_float(&self) -> Option<f64> {
        match *self {
            Value::Float(ref value) => Some(*value.value()),
            _ => None,
        }
    }

    /// Returns true iff `self` is a float.
    pub fn is_float(&self) -> bool {
        self.as_float().is_some()
    }

    /// Casts `self` to boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Boolean(ref value) => Some(*value.value()),
            _ => None,
        }
    }

    /// Returns true iff `self` is a boolean.
    pub fn is_bool(&self) -> bool {
        self.as_bool().is_some()
    }

    /// Casts `self` to str.
    pub fn as_str(&self) -> Option<&str> {
        match *self {
            Value::String(ref value) => Some(value.value()),
            _ => None,
        }
    }

    /// Returns true iff `self` is a string.
    pub fn is_str(&self) -> bool {
        self.as_str().is_some()
    }

    /// Casts `self` to date-time.
    pub fn as_date_time(&self) -> Option<&DateTime> {
        match *self {
            Value::DateTime(ref value) => Some(value.value()),
            _ => None,
        }
    }

    /// Returns true iff `self` is a date-time.
    pub fn is_date_time(&self) -> bool {
        self.as_date_time().is_some()
    }

    /// Casts `self` to array.
    pub fn as_array(&self) -> Option<&Array> {
        match *self {
            Value::Array(ref value) => Some(value),
            _ => None,
        }
    }

    /// Casts `self` to mutable array.
    pub fn as_array_mut(&mut self) -> Option<&mut Array> {
        match *self {
            Value::Array(ref mut value) => Some(value),
            _ => None,
        }
    }

    /// Returns true iff `self` is an array.
    pub fn is_array(&self) -> bool {
        self.as_array().is_some()
    }

    /// Casts `self` to inline table.
    pub fn as_inline_table(&self) -> Option<&InlineTable> {
        match *self {
            Value::InlineTable(ref value) => Some(value),
            _ => None,
        }
    }

    /// Casts `self` to mutable inline table.
    pub fn as_inline_table_mut(&mut self) -> Option<&mut InlineTable> {
        match *self {
            Value::InlineTable(ref mut value) => Some(value),
            _ => None,
        }
    }

    /// Returns true iff `self` is an inline table.
    pub fn is_inline_table(&self) -> bool {
        self.as_inline_table().is_some()
    }

    pub(crate) fn get_type(&self) -> ValueType {
        match *self {
            Value::Integer(..) => ValueType::Integer,
            Value::String(..) => ValueType::String,
            Value::Float(..) => ValueType::Float,
            Value::DateTime(..) => ValueType::DateTime,
            Value::Boolean(..) => ValueType::Boolean,
            Value::Array(..) => ValueType::Array,
            Value::InlineTable(..) => ValueType::InlineTable,
        }
    }
}

impl Value {
    /// Get the decoration of the value.
    /// # Example
    /// ```rust
    /// let v = toml_edit::Value::from(true);
    /// assert_eq!(v.decor().suffix(), "");
    ///```
    pub fn decor(&self) -> &Decor {
        match *self {
            Value::Integer(ref f) => &f.repr.decor,
            Value::String(ref f) => &f.repr.decor,
            Value::Float(ref f) => &f.repr.decor,
            Value::DateTime(ref f) => &f.repr.decor,
            Value::Boolean(ref f) => &f.repr.decor,
            Value::Array(ref a) => &a.decor,
            Value::InlineTable(ref t) => &t.decor,
        }
    }
}


pub(crate) fn sort_key_value_pairs(items: &mut KeyValuePairs) {
    let mut keys: Vec<_> = items
        .iter()
        .filter_map(|i| {
            if (i.1).value.is_sortable_name() {
                Some(i.0)
            } else {
                None
            }
        })
        .cloned()
        .collect();
    keys.sort();
    for key in keys {
        items.get_refresh(&key);
    }
}

impl FromStr for Value {
    type Err = parser::TomlError;

    /// Parses a value from a &str
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use combine::Parser;
        let parsed = parser::value_parser().easy_parse(State::new(s));
        match parsed {
            Ok((_, ref rest)) if !rest.input.is_empty() => {
                Err(Self::Err::from_unparsed(rest.positioner, s))
            }
            Ok((value, _)) => Ok(value),
            Err(e) => Err(Self::Err::new(e, s)),
        }
    }
}
