//! JSON text read into a token tape, validated by `serde_json`'s rules for a `Value`:
//! whitespace, the number grammar and range, escapes and surrogates, and the recursion
//! limit. A container's token knows how many members or items follow and where its
//! contents end, so a reader can skip a value without reading it.

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

/// Read `row` into `toks`, or reject it as `serde_json::from_str::<Value>` would.
pub(crate) fn lex(row: &str, toks: &mut Vec<Tok>) -> Result<(), Invalid> {
    toks.clear();
    let mut lexer = Lexer {
        b: row.as_bytes(),
        row,
        i: 0,
        toks,
    };
    lexer.ws();
    lexer.value(0)?;
    lexer.ws();
    if lexer.i == lexer.b.len() {
        Ok(())
    } else {
        Err(Invalid)
    }
}

struct Lexer<'a> {
    b: &'a [u8],
    row: &'a str,
    i: usize,
    toks: &'a mut Vec<Tok>,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\n' | b'\t' | b'\r') = self.peek() {
            self.i += 1;
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), Invalid> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(Invalid)
        }
    }

    fn value(&mut self, depth: usize) -> Result<(), Invalid> {
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => {
                let (start, end, escaped) = self.string()?;
                self.toks.push(Tok::Str(start, end, escaped));
                Ok(())
            }
            Some(b't') => self.literal(b"true", Tok::Bool(true)),
            Some(b'f') => self.literal(b"false", Tok::Bool(false)),
            Some(b'n') => self.literal(b"null", Tok::Null),
            Some(b'-' | b'0'..=b'9') => {
                let start = self.i;
                self.number()?;
                self.toks.push(Tok::Num(start as u32, self.i as u32));
                Ok(())
            }
            _ => Err(Invalid),
        }
    }

    fn literal(&mut self, word: &[u8], tok: Tok) -> Result<(), Invalid> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            self.toks.push(tok);
            Ok(())
        } else {
            Err(Invalid)
        }
    }

    fn array(&mut self, depth: usize) -> Result<(), Invalid> {
        if depth > MAX_DEPTH {
            return Err(Invalid);
        }
        let at = self.toks.len();
        self.toks.push(Tok::Null);
        self.i += 1;
        self.ws();
        let mut len = 0;
        if self.peek() == Some(b']') {
            self.i += 1;
        } else {
            loop {
                self.ws();
                self.value(depth)?;
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
        self.toks[at] = Tok::Arr {
            len,
            next: self.toks.len() as u32,
        };
        Ok(())
    }

    fn object(&mut self, depth: usize) -> Result<(), Invalid> {
        if depth > MAX_DEPTH {
            return Err(Invalid);
        }
        let at = self.toks.len();
        self.toks.push(Tok::Null);
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
                self.toks.push(Tok::Str(key_start, key_end, key_escaped));
                self.ws();
                self.expect(b':')?;
                self.ws();
                self.value(depth)?;
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
        self.toks[at] = Tok::Obj {
            len,
            next: self.toks.len() as u32,
            escaped,
        };
        Ok(())
    }

    /// A number in JSON's grammar; one that might be out of `f64`'s range is checked with
    /// `serde_json` itself.
    fn number(&mut self) -> Result<(), Invalid> {
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
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                digits(self);
            }
            _ => return Err(Invalid),
        }
        let mut plain = true;
        if self.peek() == Some(b'.') {
            self.i += 1;
            if digits(self) == 0 {
                return Err(Invalid);
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            plain = false;
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
        Ok(())
    }

    /// A string: the range of its text inside the quotes, and whether it holds an escape.
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
