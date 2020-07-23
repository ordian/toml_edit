use crate::decor::{Formatted, Repr};
use crate::document::Document;
use crate::table::{Item, Table};
use crate::value::{Array, DateTime, InlineTable, Value};
use crate::key::Key;
use std::str::FromStr;
use std::cell::RefCell;

use std::fmt::{Display, Formatter, Result, Write};

impl Display for Repr {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}{}{}",
            self.decor.prefix, self.raw_value, self.decor.suffix
        )
    }
}

impl<T> Display for Formatted<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.repr)
    }
}

impl Display for DateTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match *self {
            DateTime::OffsetDateTime(d) => write!(f, "{}", d),
            DateTime::LocalDateTime(d) => write!(f, "{}", d),
            DateTime::LocalDate(d) => write!(f, "{}", d),
            DateTime::LocalTime(d) => write!(f, "{}", d),
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match *self {
            Value::Integer(ref repr) => write!(f, "{}", repr),
            Value::String(ref repr) => write!(f, "{}", repr),
            Value::Float(ref repr) => write!(f, "{}", repr),
            Value::Boolean(ref repr) => write!(f, "{}", repr),
            Value::DateTime(ref repr) => write!(f, "{}", repr),
            Value::Array(ref array) => write!(f, "{}", array),
            Value::InlineTable(ref table) => write!(f, "{}", table),
        }
    }
}

impl Display for Array {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}[", self.decor.prefix)?;
        join(f, self.iter(), ",")?;
        if self.trailing_comma {
            write!(f, ",")?;
        }
        write!(f, "{}", self.trailing)?;
        write!(f, "]{}", self.decor.suffix)
    }
}

impl Display for InlineTable {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}{{", self.decor.prefix)?;
        write!(f, "{}", self.preamble)?;
        for (i, (key, value)) in self
            .items
            .iter()
            .filter(|&(_, kv)| kv.value.is_value())
            .map(|(_, kv)| (&kv.key, kv.value.as_value().unwrap()))
            .enumerate()
        {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}={}", key, value)?;
        }
        write!(f, "}}{}", self.decor.suffix)
    }
}

impl Table {
    fn visit_nested_tables<'t, F>(
        &'t self,
        path: &mut Vec<&'t str>,
        is_array_of_tables: bool,
        callback: &mut F,
    ) -> Result
    where
        F: FnMut(&Table, &Vec<&'t str>, bool) -> Result,
    {
        callback(self, path, is_array_of_tables)?;

        for kv in self.items.values() {
            match kv.value {
                Item::Table(ref t) => {
                    path.push(&kv.key.raw_value);
                    t.visit_nested_tables(path, false, callback)?;
                    path.pop();
                }
                Item::ArrayOfTables(ref a) => {
                    for t in a.iter() {
                        path.push(&kv.key.raw_value);
                        t.visit_nested_tables(path, true, callback)?;
                        path.pop();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn visit_dotted_key(
    f: &mut dyn Write,
    table: &Table,
    key: &Key
) -> Result {
    // Descend to dotted key given parts.
    let mut current = table;
    for p in &key.parts[.. key.parts.len() - 1] {
        let next = match current.get(p.get()) {
            None => return Err(std::fmt::Error),
            Some(thing) => thing
        };
        current = match next.as_table() {
            None => return Err(std::fmt::Error),
            Some(thing) => thing
        }
    }
    let real_kv = match current.items.get(key.parts[key.parts.len() - 1].get()) {
        None => return Err(std::fmt::Error),
        Some(thing) => thing
    };


    if let Item::Value(ref value) = real_kv.value {
        writeln!(f, "{}={}", real_kv.key, value)?
    }
    Ok(())
}

fn visit_table(
    f: &mut dyn Write,
    table: &Table,
    path: &[&str],
    is_array_of_tables: bool,
) -> Result {
    if path.is_empty() {
        // don't print header for the root node
    } else if is_array_of_tables {
        write!(f, "{}[[", table.decor.prefix)?;
        write!(f, "{}", path.join("."))?;
        writeln!(f, "]]{}", table.decor.suffix)?;
    } else if !(table.implicit && table.values_len() == 0) &&
              !(table.implicit && table.has_dotted) {
        write!(f, "{}[", table.decor.prefix)?;
        write!(f, "{}", path.join("."))?;
        writeln!(f, "]{}", table.decor.suffix)?;
    }
    // print table body
    for kv in table.items.values() {
        let key = Key::from_str(&kv.key.raw_value).unwrap();
        if let Item::Value(ref value) = kv.value {
            // If this key is a dotted key it should have been 
            // written out in the parent table of first simple-key
            // of the dotted key.
            if !key.is_dotted_key() {
                writeln!(f, "{}={}", kv.key, value)?;
            }
        } else if let Item::DottedKeyMarker(_) = kv.value {
            if let Err(_e) = visit_dotted_key(f, table, &key) {
                eprintln!("Something went wrong with dotted key {}, continuing.", key.get());
            }
        }
    }
    Ok(())
}

impl Display for Table {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut path = Vec::new();

        self.visit_nested_tables(&mut path, false, &mut |t, path, is_array| {
            visit_table(f, t, path, is_array)
        })?;
        Ok(())
    }
}

impl Document {
    /// Returns a string representation of the TOML document, attempting to keep
    /// the table headers in their original order.
    pub fn to_string_in_original_order(&self) -> String {
        let mut string = String::new();
        let mut path = Vec::new();
        let mut last_position = 0;
        let mut tables = Vec::new();
        self.as_table()
            .visit_nested_tables(&mut path, false, &mut |t, p, is_array| {
                if let Some(pos) = t.position {
                    last_position = pos;
                }
                let mut s = String::new();
                visit_table(&mut s, t, p, is_array)?;
                tables.push((last_position, s));
                Ok(())
            })
            .unwrap();

        tables.sort_by_key(|&(id, _)| id);
        for (_, table) in tables {
            string.push_str(&table);
        }
        string.push_str(&self.trailing);
        string
    }
}

impl Display for Document {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.as_table())?;
        write!(f, "{}", self.trailing)
    }
}

fn join<D, I>(f: &mut Formatter<'_>, iter: I, sep: &str) -> Result
where
    D: Display,
    I: Iterator<Item = D>,
{
    for (i, v) in iter.enumerate() {
        if i > 0 {
            write!(f, "{}", sep)?;
        }
        write!(f, "{}", v)?;
    }
    Ok(())
}
