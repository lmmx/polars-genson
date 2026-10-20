//! JSON text read by `serde_json`'s rules for a `Value`: whitespace, the number grammar and
//! range, escapes and surrogates, and the recursion limit. The lexer hands what it reads
//! to a [`Sink`] as it reads it: into a token tape (`lex`), where a container's token knows
//! how many members or items follow and where its contents end, so a reader can skip a
//! value without reading it; or, in inference, straight into the values' interned states,
//! with no tape between. Every sink is given the same text checked by the same rules.

use crate::extract::scan::{plain_prefix, HEX};
use serde_json::Value;

/// The `Value` path's recursion limit: `serde_json` rejects a 128th level of nesting.
const MAX_DEPTH: usize = 127;

/// A token of a row's text. A container is followed by its contents (an object by each
/// member's key and value in turn), and knows the index of the token after its contents.
#[derive(Clone, Copy, Debug)]
// A boolean's value and an object's escapes are read by the typed decoder alone
#[cfg_attr(not(feature = "parquet"), allow(dead_code))]
pub(crate) enum Tok {
    Null,
    Bool(bool),
    /// A number's text
    Num(u32, u32),
    /// A string's text inside its quotes, and whether it holds an escape
    Str(u32, u32, bool),
    Arr {
        len: u32,
        next: u32,
    },
    /// An object, with whether a key in it has an escape (its keys are then decoded to be
    /// compared as `Value` keys are)
    Obj {
        len: u32,
        next: u32,
        escaped: bool,
    },
}

/// Text `serde_json::from_str::<Value>` rejects.
pub(crate) struct Invalid;

/// What a reading of a row makes of the values the lexer finds in it, in the order of the
/// text: a value's parts before the value (a container's contents, a member's key before
/// its value). A sink may refuse a value (`Invalid`), which ends the reading.
pub(crate) trait Sink {
    /// What a value is read into
    type Value;
    /// A list's reading while its items are read
    type List;
    /// An object's reading while its members are read
    type Object;
    fn null(&mut self) -> Self::Value;
    fn boolean(&mut self, value: bool) -> Self::Value;
    /// A string's text inside its quotes, and whether it holds an escape
    fn string(&mut self, start: u32, end: u32, escaped: bool) -> Self::Value;
    /// A number's text, and whether it is an integer of 18 digits or fewer (which an
    /// `i64` holds)
    fn number(&mut self, start: u32, end: u32, short_integer: bool)
        -> Result<Self::Value, Invalid>;
    fn open_list(&mut self) -> Self::List;
    fn item(&mut self, list: &mut Self::List, item: Self::Value);
    fn close_list(&mut self, list: Self::List, len: u32) -> Result<Self::Value, Invalid>;
    fn open_object(&mut self) -> Self::Object;
    /// A member's key, before its value
    fn key(&mut self, object: &mut Self::Object, start: u32, end: u32, escaped: bool);
    fn member(&mut self, object: &mut Self::Object, value: Self::Value);
    /// An object read whole, with whether a key in it has an escape
    fn close_object(&mut self, object: Self::Object, len: u32, escaped: bool) -> Self::Value;
}

/// Read `row` into `toks`, or reject it as `serde_json::from_str::<Value>` would.
pub(crate) fn lex(row: &str, toks: &mut Vec<Tok>) -> Result<(), Invalid> {
    toks.clear();
    read(row, &mut Tape(toks))
}

/// Read `row` into `sink`, or reject it as `serde_json::from_str::<Value>` would (or as the
/// sink refuses it).
pub(crate) fn read<S: Sink>(row: &str, sink: &mut S) -> Result<S::Value, Invalid> {
    let mut lexer = Lexer {
        b: row.as_bytes(),
        row,
        i: 0,
        sink,
    };
    lexer.ws();
    let value = lexer.value(0)?;
    lexer.ws();
    if lexer.i == lexer.b.len() {
        Ok(value)
    } else {
        Err(Invalid)
    }
}

/// A row's token tape.
struct Tape<'t>(&'t mut Vec<Tok>);

impl Sink for Tape<'_> {
    type Value = ();
    /// The index of the container's token, written once its contents are read
    type List = usize;
    type Object = usize;

    fn null(&mut self) {
        self.0.push(Tok::Null);
    }

    fn boolean(&mut self, value: bool) {
        self.0.push(Tok::Bool(value));
    }

    fn string(&mut self, start: u32, end: u32, escaped: bool) {
        self.0.push(Tok::Str(start, end, escaped));
    }

    fn number(&mut self, start: u32, end: u32, _: bool) -> Result<(), Invalid> {
        self.0.push(Tok::Num(start, end));
        Ok(())
    }

    fn open_list(&mut self) -> usize {
        self.0.push(Tok::Null);
        self.0.len() - 1
    }

    fn item(&mut self, _: &mut usize, _: ()) {}

    fn close_list(&mut self, at: usize, len: u32) -> Result<(), Invalid> {
        let next = self.0.len() as u32;
        self.0[at] = Tok::Arr { len, next };
        Ok(())
    }

    fn open_object(&mut self) -> usize {
        self.0.push(Tok::Null);
        self.0.len() - 1
    }

    fn key(&mut self, _: &mut usize, start: u32, end: u32, escaped: bool) {
        self.0.push(Tok::Str(start, end, escaped));
    }

    fn member(&mut self, _: &mut usize, _: ()) {}

    fn close_object(&mut self, at: usize, len: u32, escaped: bool) {
        let next = self.0.len() as u32;
        self.0[at] = Tok::Obj { len, next, escaped };
    }
}

struct Lexer<'a, S> {
    b: &'a [u8],
    row: &'a str,
    i: usize,
    sink: &'a mut S,
}

impl<S: Sink> Lexer<'_, S> {
    #[inline]
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    #[inline]
    fn ws(&mut self) {
        while let Some(b' ' | b'\n' | b'\t' | b'\r') = self.peek() {
            self.i += 1;
        }
    }

    #[inline]
    fn expect(&mut self, c: u8) -> Result<(), Invalid> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(Invalid)
        }
    }

    fn value(&mut self, depth: usize) -> Result<S::Value, Invalid> {
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => {
                let (start, end, escaped) = self.string()?;
                Ok(self.sink.string(start, end, escaped))
            }
            Some(b't') => {
                self.literal(b"true")?;
                Ok(self.sink.boolean(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(self.sink.boolean(false))
            }
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(self.sink.null())
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.i;
                let short_integer = self.number()?;
                self.sink.number(start as u32, self.i as u32, short_integer)
            }
            _ => Err(Invalid),
        }
    }

    #[inline]
    fn literal(&mut self, word: &[u8]) -> Result<(), Invalid> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(())
        } else {
            Err(Invalid)
        }
    }

    fn array(&mut self, depth: usize) -> Result<S::Value, Invalid> {
        if depth > MAX_DEPTH {
            return Err(Invalid);
        }
        let mut list = self.sink.open_list();
        self.i += 1;
        self.ws();
        let mut len = 0;
        if self.peek() == Some(b']') {
            self.i += 1;
        } else {
            loop {
                self.ws();
                let item = self.value(depth)?;
                self.sink.item(&mut list, item);
                len += 1;
                self.ws();
                match self.peek() {
                    Some(b',') => self.i += 1,
                    Some(b']') => {
                        self.i += 1;
                        break;
                    }
                    _ => return Err(Invalid),
                }
            }
        }
        self.sink.close_list(list, len)
    }

    fn object(&mut self, depth: usize) -> Result<S::Value, Invalid> {
        if depth > MAX_DEPTH {
            return Err(Invalid);
        }
        let mut object = self.sink.open_object();
        self.i += 1;
        self.ws();
        let mut len = 0;
        let mut escaped = false;
        if self.peek() == Some(b'}') {
            self.i += 1;
        } else {
            loop {
                self.ws();
                let (key_start, key_end, key_escaped) = self.string()?;
                escaped |= key_escaped;
                self.sink.key(&mut object, key_start, key_end, key_escaped);
                self.ws();
                self.expect(b':')?;
                self.ws();
                let value = self.value(depth)?;
                self.sink.member(&mut object, value);
                len += 1;
                self.ws();
                match self.peek() {
                    Some(b',') => self.i += 1,
                    Some(b'}') => {
                        self.i += 1;
                        break;
                    }
                    _ => return Err(Invalid),
                }
            }
        }
        Ok(self.sink.close_object(object, len, escaped))
    }

    /// A number in JSON's grammar; one that might be out of `f64`'s range is checked with
    /// `serde_json` itself. Whether it is an integer of 18 digits or fewer.
    fn number(&mut self) -> Result<bool, Invalid> {
        let start = self.i;
        let digits = |s: &mut Self| {
            let from = s.i;
            while let Some(b'0'..=b'9') = s.peek() {
                s.i += 1;
            }
            s.i - from
        };
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let whole = match self.peek() {
            Some(b'0') => {
                self.i += 1;
                1
            }
            Some(b'1'..=b'9') => digits(self),
            _ => return Err(Invalid),
        };
        let mut plain = true;
        let mut integer = true;
        if self.peek() == Some(b'.') {
            integer = false;
            self.i += 1;
            if digits(self) == 0 {
                return Err(Invalid);
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            plain = false;
            integer = false;
            self.i += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.i += 1;
            }
            if digits(self) == 0 {
                return Err(Invalid);
            }
        }
        // Without an exponent, only a very long number can be out of range
        if !plain || self.i - start > 300 {
            let token = &self.row[start..self.i];
            if serde_json::from_str::<Value>(token).is_err() {
                return Err(Invalid);
            }
        }
        Ok(integer && whole <= 18)
    }

    /// A string: the range of its text inside the quotes, and whether it holds an escape.
    #[inline]
    fn string(&mut self) -> Result<(u32, u32, bool), Invalid> {
        self.expect(b'"')?;
        let start = self.i;
        let mut escaped = false;
        loop {
            self.i += plain_prefix(&self.b[self.i..]);
            match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    return Ok((start as u32, self.i as u32 - 1, escaped));
                }
                Some(b'\\') => {
                    escaped = true;
                    // A run of the common escape, `\uXXXX` outside the surrogates
                    while let Some(&[b'\\', b'u', d0, d1, d2, d3]) = self.b.get(self.i..self.i + 6)
                    {
                        let [a, b, c, e] = [d0, d1, d2, d3].map(|x| HEX[x as usize]);
                        let code = u16::from(a) << 12
                            | u16::from(b) << 8
                            | u16::from(c) << 4
                            | u16::from(e);
                        if (a | b | c | e) & 0xF0 != 0 || (0xD800..=0xDFFF).contains(&code) {
                            break;
                        }
                        self.i += 6;
                    }
                    if self.peek() == Some(b'\\') {
                        self.i += 1;
                        self.escape()?;
                    }
                }
                // A control character, or the end of the row
                _ => return Err(Invalid),
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, Invalid> {
        let d = self.b.get(self.i..self.i + 4).ok_or(Invalid)?;
        let [a, b, c, e] = [d[0], d[1], d[2], d[3]].map(|x| HEX[x as usize]);
        if (a | b | c | e) & 0xF0 != 0 {
            return Err(Invalid);
        }
        self.i += 4;
        Ok(u16::from(a) << 12 | u16::from(b) << 8 | u16::from(c) << 4 | u16::from(e))
    }

    /// The escape after a backslash, with `serde_json`'s rules for surrogates: a high one
    /// must be followed by a `\u` low one, and a low one may not stand alone.
    fn escape(&mut self) -> Result<(), Invalid> {
        let Some(c) = self.peek() else {
            return Err(Invalid);
        };
        self.i += 1;
        match c {
            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => Ok(()),
            b'u' => match self.hex4()? {
                0xD800..=0xDBFF => {
                    self.expect(b'\\')?;
                    self.expect(b'u')?;
                    match self.hex4()? {
                        0xDC00..=0xDFFF => Ok(()),
                        _ => Err(Invalid),
                    }
                }
                0xDC00..=0xDFFF => Err(Invalid),
                _ => Ok(()),
            },
            _ => Err(Invalid),
        }
    }
}

/// The text of a string token holding an escape (`Tok::Str`'s range, inside the quotes),
/// unescaped as `serde_json` reads it. The lexer has checked every escape
/// (`Lexer::escape`), so each is whole and a high surrogate is followed by a low one.
pub(crate) fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    unescape_into(raw, &mut out);
    out
}

/// `unescape` written to `out` (which cannot fail: a `String`, or a string builder's
/// value), so that it need not be built apart first. At most `raw.len()` bytes.
pub(crate) fn unescape_into(raw: &str, out: &mut impl std::fmt::Write) {
    let b = raw.as_bytes();
    let hex4 = |i: usize| {
        b[i..i + 4]
            .iter()
            .fold(0u32, |n, &d| n << 4 | u32::from(HEX[d as usize]))
    };
    let mut i = 0;
    loop {
        let run = plain_prefix(&b[i..]);
        let _ = out.write_str(&raw[i..i + run]);
        i += run;
        if i == b.len() {
            return;
        }
        let c = match b[i + 1] {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            _ => {
                let mut code = hex4(i + 2);
                i += 6;
                if (0xD800..0xDC00).contains(&code) {
                    code = 0x10000 + ((code - 0xD800) << 10) + (hex4(i + 2) - 0xDC00);
                    i += 6;
                }
                let _ = out.write_char(char::from_u32(code).expect("a checked escape"));
                continue;
            }
        };
        let _ = out.write_char(c);
        i += 2;
    }
}

#[cfg(test)]
mod unescape_tests {
    use super::*;

    /// Random strings of every escape (surrogate pairs and `\u0000` among them) beside
    /// ASCII and other characters, as the lexer takes them: each unescapes as `serde_json`
    /// reads it.
    #[test]
    fn test_unescape_matches_serde_json() {
        let pieces = [
            r#"\""#, r"\\", r"\/", r"\b", r"\f", r"\n", r"\r", r"\t", r"A", r"é", r"中", r"😀",
            r"􏿿", r"\u0000", r"￿", "a", "xyz", "é", "中", "😀", " ", "12345678",
        ];
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..20_000 {
            let s: String = (0..next() % 14)
                .map(|_| pieces[(next() % pieces.len() as u64) as usize])
                .collect();
            let quoted = format!("\"{s}\"");
            assert!(lex(&quoted, &mut Vec::new()).is_ok(), "{quoted}");
            let expected: String = serde_json::from_str(&quoted).unwrap();
            assert_eq!(unescape(&s), expected, "{s}");
        }
    }
}
