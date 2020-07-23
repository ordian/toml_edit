use crate::decor::{InternalString};
use crate::formatted::decorated;
use crate::parser::errors::CustomError;
use crate::parser::key::key_path2;
use crate::parser::trivia::ws;
use crate::parser::value::value;
use crate::table::{Item, TableKeyValue};
use crate::decor::Repr;
use crate::value::InlineTable;
use combine::char::char;
use combine::stream::RangeStream;
use combine::*;

// ;; Inline Table

// inline-table = inline-table-open inline-table-keyvals inline-table-close
parse!(inline_table() -> InlineTable, {
    between(char(INLINE_TABLE_OPEN), char(INLINE_TABLE_CLOSE),
            inline_table_keyvals().and_then(|(p, v)| table_from_pairs(p, v)))
});

fn table_from_pairs(
    preamble: &str,
    v: Vec<(InternalString, TableKeyValue)>,
) -> Result<InlineTable, CustomError> {
    let mut table = InlineTable::default();
    table.preamble = InternalString::from(preamble);

    for (k, kv) in v {
        if table.contains_key(&k) {
            return Err(CustomError::DuplicateKey {
                key: k,
                table: "inline".into(),
            });
        }
        table.items.insert(k, kv);
    }
    Ok(table)
}

// inline-table-open  = %x7B ws     ; {
const INLINE_TABLE_OPEN: char = '{';
// inline-table-close = ws %x7D     ; }
const INLINE_TABLE_CLOSE: char = '}';
// inline-table-sep   = ws %x2C ws  ; , Comma
const INLINE_TABLE_SEP: char = ',';
// keyval-sep = ws %x3D ws ; =
pub(crate) const KEYVAL_SEP: char = '=';

// inline-table-keyvals = [ inline-table-keyvals-non-empty ]
// inline-table-keyvals-non-empty =
// ( key keyval-sep val inline-table-sep inline-table-keyvals-non-empty ) /
// ( key keyval-sep val )

parse!(inline_table_keyvals() -> (&'a str, Vec<(InternalString, TableKeyValue)>), {
    (
        sep_by(keyval(), char(INLINE_TABLE_SEP)),
        ws(),
    ).map(|(v, w)| {
        (w, v)
    })
});

parse!(keyval() -> (InternalString, TableKeyValue), {
    (
        attempt(key_path2()),
        char(KEYVAL_SEP),
        (ws(), value(), ws()),
    ).map(|(key, _, v)| {
        let (pre, v, suf) = v;
        let v = decorated(v, pre, suf);

        (
            key.get(),
            TableKeyValue {
                // At least one, TODO: fix me for dotted.
                key: Repr::new(key.parts[0].decor.prefix.clone(), key.parts[0].raw.clone(), key.parts[0].decor.suffix.clone()),
                value: Item::Value(v),
            }
        )
    })
});
