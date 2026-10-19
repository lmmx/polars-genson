//! `extract_invariants` on a row's text, without building its `Value`.
//!
//! One pass over the row validates it as `serde_json::from_str::<Value>` would and finds the
//! fields that `strip` removes, as byte ranges: the row with them cut out is the slim row,
//! and each removed field's text is its lookup value, parsed only where it is not already
//! known. Anything the pass does not handle exactly as `strip` and a re-serialisation would
//! makes it give up on the row ([`Fallback`]), which then takes the `Value` path:
//!
//! - text that is not valid JSON, or valid only by a rule not checked here;
//! - a line break (each slim row is one line: inference may read rows as NDJSON);
//! - a key with an escape, or a key twice in one object, in an object that is kept;
//! - a field to extract whose determinant is missing or null: the field stays, and its
//!   value, only validated here, would have to be looked into.

use rustc_hash::FxHashSet;
use std::ops::Range;

/// The pass gave up on a row (see the module docs); the row takes the `Value` path.
#[derive(Debug)]
pub(crate) struct Fallback;

/// A field removed from a row: its index in the spec, its determinant's value, and the
/// byte range of its value in the row.
pub(crate) struct Removed<'a> {
    pub field: usize,
    pub key: std::borrow::Cow<'a, str>,
    pub value: Range<usize>,
}

/// A row's slim text and its removed fields, in the order `strip` records them.
pub(crate) struct Scanned<'a> {
    pub slim: String,
    pub removed: Vec<Removed<'a>>,
}

/// The `Value` path's recursion limit: `serde_json` rejects a 128th level of nesting.
const MAX_DEPTH: usize = 127;

/// Scan `row` for the `spec` fields (`(field, determinant)` pairs).
pub(crate) fn scan<'a>(row: &'a str, spec: &[(String, String)]) -> Result<Scanned<'a>, Fallback> {
    let bytes = row.as_bytes();
    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return Err(Fallback);
    }
    let mut s = Scanner {
        b: bytes,
        row,
        i: 0,
        spec,
        cuts: Vec::new(),
        removed: Vec::new(),
        members: Vec::new(),
    };
    s.ws();
    s.value(0)?;
    s.ws();
    if s.i != bytes.len() {
        return Err(Fallback);
    }
    let mut cuts = s.cuts;
    cuts.sort_by_key(|c| c.start);
    let cut: usize = cuts.iter().map(|c| c.len()).sum();
    let mut slim = String::with_capacity(row.len() - cut);
    let mut at = 0;
    for cut in &cuts {
        slim.push_str(&row[at..cut.start]);
        at = cut.end;
    }
    slim.push_str(&row[at..]);
    Ok(Scanned {
        slim,
        removed: s.removed,
    })
}

/// One member of an object being scanned: its key, the start of its key's opening quote,
/// the range of its value, and whether its value was scanned (not only validated).
struct Member {
    key: Range<usize>,
    start: usize,
    value: Range<usize>,
}

struct Scanner<'a, 's> {
    b: &'a [u8],
    row: &'a str,
    i: usize,
    spec: &'s [(String, String)],
    cuts: Vec<Range<usize>>,
    removed: Vec<Removed<'a>>,
    /// The members of the objects being scanned, innermost last.
    members: Vec<Member>,
}

impl<'a> Scanner<'a, '_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t') = self.peek() {
            self.i += 1;
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), Fallback> {
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(Fallback)
        }
    }

    /// A value whose objects are looked into for fields to extract.
    fn value(&mut self, depth: usize) -> Result<(), Fallback> {
        match self.peek() {
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => {
                if depth + 1 > MAX_DEPTH {
                    return Err(Fallback);
                }
                self.i += 1;
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(());
                }
                loop {
                    self.ws();
                    self.value(depth + 1)?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(());
                        }
                        _ => return Err(Fallback),
                    }
                }
            }
            _ => self.scalar(),
        }
    }

    /// A value that is only validated: the value of a field to extract.
    fn skip(&mut self, depth: usize) -> Result<(), Fallback> {
        match self.peek() {
            Some(open @ (b'{' | b'[')) => {
                if depth + 1 > MAX_DEPTH {
                    return Err(Fallback);
                }
                self.i += 1;
                self.ws();
                let close = if open == b'{' { b'}' } else { b']' };
                if self.peek() == Some(close) {
                    self.i += 1;
                    return Ok(());
                }
                loop {
                    self.ws();
                    if open == b'{' {
                        self.string()?;
                        self.ws();
                        self.expect(b':')?;
                        self.ws();
                    }
                    self.skip(depth + 1)?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(c) if c == close => {
                            self.i += 1;
                            return Ok(());
                        }
                        _ => return Err(Fallback),
                    }
                }
            }
            _ => self.scalar(),
        }
    }

    fn scalar(&mut self) -> Result<(), Fallback> {
        match self.peek() {
            Some(b'"') => self.string().map(|_| ()),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(Fallback),
        }
    }

    fn literal(&mut self, word: &[u8]) -> Result<(), Fallback> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(())
        } else {
            Err(Fallback)
        }
    }

    /// A number in JSON's grammar; one that might be out of `f64`'s range is checked with
    /// `serde_json` itself.
    fn number(&mut self) -> Result<(), Fallback> {
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
            _ => return Err(Fallback),
        }
        let mut plain = true;
        if self.peek() == Some(b'.') {
            self.i += 1;
            if digits(self) == 0 {
                return Err(Fallback);
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            plain = false;
            self.i += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.i += 1;
            }
            if digits(self) == 0 {
                return Err(Fallback);
            }
        }
        // Without an exponent, only a very long number can be out of range
        if !plain || self.i - start > 300 {
            let token = &self.row[start..self.i];
            if serde_json::from_str::<serde_json::Value>(token).is_err() {
                return Err(Fallback);
            }
        }
        Ok(())
    }

    /// A string; returns whether it holds an escape.
    fn string(&mut self) -> Result<bool, Fallback> {
        self.expect(b'"')?;
        let mut escaped = false;
        loop {
            self.i += plain_prefix(&self.b[self.i..]);
            match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(escaped);
                }
                Some(b'\\') => {
                    escaped = true;
                    // A run of the common escape, `\uXXXX` outside the surrogates, read in
                    // place (text in most scripts, as `json.dumps` writes it)
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
                _ => return Err(Fallback),
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, Fallback> {
        let d = self.b.get(self.i..self.i + 4).ok_or(Fallback)?;
        let [a, b, c, e] = [
            HEX[d[0] as usize],
            HEX[d[1] as usize],
            HEX[d[2] as usize],
            HEX[d[3] as usize],
        ];
        if (a | b | c | e) & 0xF0 != 0 {
            return Err(Fallback);
        }
        self.i += 4;
        Ok(u16::from(a) << 12 | u16::from(b) << 8 | u16::from(c) << 4 | u16::from(e))
    }

    /// The escape after a backslash, with `serde_json`'s rules for surrogates: a high one
    /// must be followed by a `\u` low one, and a low one may not stand alone.
    fn escape(&mut self) -> Result<(), Fallback> {
        let Some(c) = self.peek() else {
            return Err(Fallback);
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
                        _ => Err(Fallback),
                    }
                }
                0xDC00..=0xDFFF => Err(Fallback),
                _ => Ok(()),
            },
            _ => Err(Fallback),
        }
    }

    /// An object: its members are scanned (or, for a field to extract, validated), then the
    /// fields `strip` removes are cut from it and recorded, before those of its members.
    fn object(&mut self, depth: usize) -> Result<(), Fallback> {
        if depth > MAX_DEPTH {
            return Err(Fallback);
        }
        self.i += 1;
        let first = self.members.len();
        let removed_before = self.removed.len();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(());
        }
        loop {
            self.ws();
            let start = self.i;
            if self.string()? {
                return Err(Fallback);
            }
            let key = start + 1..self.i - 1;
            self.ws();
            self.expect(b':')?;
            self.ws();
            let value_start = self.i;
            let name = &self.row[key.clone()];
            if self.spec.iter().any(|(field, _)| field == name) {
                self.skip(depth)?;
            } else {
                self.value(depth)?;
            }
            self.members.push(Member {
                key,
                start,
                value: value_start..self.i,
            });
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(Fallback),
            }
        }
        let result = self.close_object(first, removed_before);
        self.members.truncate(first);
        result
    }

    fn close_object(&mut self, first: usize, removed_before: usize) -> Result<(), Fallback> {
        let row = self.row;
        let members = &self.members[first..];
        let name = |m: &Member| &row[m.key.clone()];
        // A repeated key: `Value` keeps the last value at the first place
        if members.len() <= 16 {
            for (i, m) in members.iter().enumerate() {
                if members[..i].iter().any(|n| name(n) == name(m)) {
                    return Err(Fallback);
                }
            }
        } else {
            let mut seen = FxHashSet::default();
            if !members.iter().all(|m| seen.insert(name(m))) {
                return Err(Fallback);
            }
        }
        // `strip`: each spec field in turn, when its determinant is present and not null
        let mut gone = vec![false; members.len()];
        let mut own = Vec::new();
        for (field_index, (field, key_field)) in self.spec.iter().enumerate() {
            let find = |n: &str, gone: &[bool]| {
                members
                    .iter()
                    .enumerate()
                    .position(|(j, m)| !gone[j] && name(m) == n)
            };
            let Some(f) = find(field, &gone) else {
                continue;
            };
            let key = match find(key_field, &gone) {
                Some(k) => determinant(&row[members[k].value.clone()])?,
                None => None,
            };
            let Some(key) = key else {
                // The field stays, and its value was only validated
                return Err(Fallback);
            };
            gone[f] = true;
            own.push(Removed {
                field: field_index,
                key,
                value: members[f].value.clone(),
            });
        }
        if own.is_empty() {
            return Ok(());
        }
        // Spec fields left in the object would have been looked into
        if members
            .iter()
            .enumerate()
            .any(|(j, m)| !gone[j] && self.spec.iter().any(|(field, _)| field == name(m)))
        {
            return Err(Fallback);
        }
        // Cut each run of removed members: up to the next kept member, or, at the end, from
        // the previous kept member's end (taking the comma before the run)
        let mut cuts = Vec::new();
        let mut j = 0;
        while j < members.len() {
            if !gone[j] {
                j += 1;
                continue;
            }
            let run_start = j;
            while j < members.len() && gone[j] {
                j += 1;
            }
            let cut = match (j < members.len(), run_start.checked_sub(1)) {
                (true, _) => members[run_start].start..members[j].start,
                (false, Some(prev)) => members[prev].value.end..members[j - 1].value.end,
                (false, None) => members[run_start].start..members[j - 1].value.end,
            };
            cuts.push(cut);
        }
        self.cuts.extend(cuts);
        // This object's fields go before those found in its members
        self.removed.splice(removed_before..removed_before, own);
        Ok(())
    }
}

/// Each byte's value as a hex digit, or 0xFF.
static HEX: [u8; 256] = {
    let mut table = [0xFF; 256];
    let mut i = 0;
    while i < 10 {
        table[b'0' as usize + i] = i as u8;
        i += 1;
    }
    let mut i = 0;
    while i < 6 {
        table[b'a' as usize + i] = 10 + i as u8;
        table[b'A' as usize + i] = 10 + i as u8;
        i += 1;
    }
    table
};

/// How many bytes at the start of `s` are none of a quote, a backslash and a control
/// character, read eight at a time: in a word, the lowest byte that is zero (or below a
/// bound) is flagged exactly, so its index is the count.
fn plain_prefix(s: &[u8]) -> usize {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGHS: u64 = 0x8080_8080_8080_8080;
    let zero = |v: u64| v.wrapping_sub(ONES) & !v & HIGHS;
    let mut i = 0;
    while let Some(chunk) = s.get(i..i + 8) {
        let w = u64::from_le_bytes(chunk.try_into().unwrap());
        let found = zero(w ^ (ONES * u64::from(b'"')))
            | zero(w ^ (ONES * u64::from(b'\\')))
            | (w.wrapping_sub(ONES * 0x20) & !w & HIGHS);
        if found != 0 {
            return i + (found.trailing_zeros() / 8) as usize;
        }
        i += 8;
    }
    while let Some(&c) = s.get(i) {
        if c == b'"' || c == b'\\' || c < 0x20 {
            break;
        }
        i += 1;
    }
    i
}

/// A determinant's lookup key, as `strip` reads it: a string's content, `None` for null,
/// and any other value's JSON.
fn determinant(text: &str) -> Result<Option<std::borrow::Cow<'_, str>>, Fallback> {
    match text.as_bytes().first() {
        Some(b'n') => Ok(None),
        Some(b'"') if !text.contains('\\') => Ok(Some(text[1..text.len() - 1].into())),
        _ => {
            let value: serde_json::Value = serde_json::from_str(text).map_err(|_| Fallback)?;
            Ok(Some(match value {
                serde_json::Value::String(s) => s.into(),
                other => other.to_string().into(),
            }))
        }
    }
}
