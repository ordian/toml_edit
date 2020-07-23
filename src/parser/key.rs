use crate::decor::InternalString;
use crate::parser::strings::{basic_string, literal_string};
use crate::parser::trivia::ws;
use crate::decor::Decor;
use crate::key::{Key, SimpleKey};

use combine::range::{recognize_with_value, take_while1};
use combine::stream::RangeStream;
use combine::*;
use combine::char::char;
// use combine::skip_count;

#[inline]
fn is_unquoted_char(c: char) -> bool {
    match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' => true,
        _ => false,
    }
}

// unquoted-key = 1*( ALPHA / DIGIT / %x2D / %x5F ) ; A-Z / a-z / 0-9 / - / _
parse!(unquoted_key() -> &'a str, {
    take_while1(is_unquoted_char)
});

// From spec:
// simple-key = quoted-key / unquoted-key
// actual: key = unquoted-key / basic-string / literal-string

parse!(simple_key() -> (&'a str, (&'a str, InternalString), &'a str), {
    (
        ws(),
        recognize_with_value(
            choice((
                basic_string(),
                literal_string().map(|s: &'a str| s.into()),
                unquoted_key().map(|s: &'a str| s.into()),
            ))
        ),
        ws()
    )
});

const DOT_SEP: char = '.';

// dotted-key = simple-key 1*( dot-sep simple-key )
// key = simple-key / dotted-key
parse!(key() -> (&'a str, Vec<SimpleKey>), {
    recognize_with_value(
        (
            simple_key(),
            many::<Vec<_>, _>(char(DOT_SEP).with(simple_key()))
        ).map(|(first, rest)| {
            let (pre, (raw, val), suf) = first;
            // same as raw = pre.to_string() + &val + suf?
            let mut keys = vec![SimpleKey::new(Decor::new(pre.clone(), suf.clone()), raw.into(), val.clone())];
            let mut more_keys: Vec<_> = rest.iter().map(|r: &(&str, (&str, InternalString), &str)| {
                let (pre, (raw, val), suf) = r;
                SimpleKey::new(Decor::new(pre.clone(), suf.clone()), raw.to_string(), val.clone())
            }).collect();
            keys.append(&mut more_keys);

            keys
        })
    )
});

parse!(key_path2() -> Key, {
    key().map(|(raw, parts)| Key::new(raw.into(), parts))
});