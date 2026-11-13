//! SQLite 3 数据库（GeoPackage 也是它）：照公开的文件格式说明读 B 树，不执行任何 SQL。
//! 每张表一个标题和一个 Markdown 表格；列名从 `sqlite_schema` 的建表语句里取。
//! 页、行、输出都有上限；循环引用的页只走一次。

use std::collections::HashSet;

use crate::md::{self, Block};
use crate::{Converted, Error, Options, budget, check_deadline};

pub(crate) const MAGIC: &[u8] = b"SQLite format 3\0";

/// 单个记录最多拼这么大（溢出页可以声明很长）。
const MAX_PAYLOAD: usize = 64 << 20;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(usize),
}

impl Value {
    fn display(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Int(v) => v.to_string(),
            Value::Real(v) => v.to_string(),
            Value::Text(s) => s.clone(),
            Value::Blob(n) => format!("<blob {n} bytes>"),
        }
    }
}

struct Db<'a> {
    data: &'a [u8],
    page_size: usize,
    usable: usize,
    /// 0 UTF-8、1 UTF-16LE、2 UTF-16BE。
    encoding: u8,
}

fn varint(d: &[u8], at: usize) -> Option<(u64, usize)> {
    let mut v = 0u64;
    for i in 0..8 {
        let b = *d.get(at + i)?;
        v = v << 7 | u64::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Some((v, at + i + 1));
        }
    }
    let b = *d.get(at + 8)?;
    Some((v << 8 | u64::from(b), at + 9))
}

fn be16(d: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(u16::from_be_bytes(
        d.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

impl<'a> Db<'a> {
    fn open(data: &'a [u8]) -> Result<Db<'a>, Error> {
        let bad = || Error::Malformed("broken SQLite header".into());
        if !data.starts_with(MAGIC) {
            return Err(bad());
        }
        let raw = be16(data, 16).ok_or_else(bad)?;
        let page_size = if raw == 1 { 65_536 } else { raw };
        if page_size < 512 || !page_size.is_power_of_two() {
            return Err(bad());
        }
        let reserved = usize::from(*data.get(20).ok_or_else(bad)?);
        let usable = page_size
            .checked_sub(reserved)
            .filter(|u| *u >= 480)
            .ok_or_else(bad)?;
        let encoding = match be32(data, 56).ok_or_else(bad)? {
            2 => 1,
            3 => 2,
            _ => 0,
        };
        Ok(Db {
            data,
            page_size,
            usable,
            encoding,
        })
    }

    fn page(&self, n: u32) -> Option<&'a [u8]> {
        let at = (n as usize).checked_sub(1)?.checked_mul(self.page_size)?;
        self.data.get(at..at.checked_add(self.page_size)?)
    }

    /// 拼出完整负载：本页放不下的部分在溢出页链上。
    fn payload(&self, page: &[u8], at: usize, size: usize, index: bool) -> Option<Vec<u8>> {
        if size > MAX_PAYLOAD {
            return None;
        }
        let u = self.usable;
        let x = if index {
            (u - 12) * 64 / 255 - 23
        } else {
            u - 35
        };
        if size <= x {
            return page.get(at..at + size).map(<[u8]>::to_vec);
        }
        let m = (u - 12) * 32 / 255 - 23;
        let k = m + (size - m) % (u - 4);
        let local = if k <= x { k } else { m };
        let mut out = page.get(at..at + local)?.to_vec();
        let mut next = be32(page, at + local)?;
        let mut seen = HashSet::new();
        while out.len() < size && next != 0 {
            if !seen.insert(next) {
                return None;
            }
            let p = self.page(next)?;
            next = be32(p, 0)?;
            let take = (size - out.len()).min(u - 4);
            out.extend_from_slice(p.get(4..4 + take)?);
        }
        Some(out)
    }

    /// 按顺序走一棵 B 树。`visit` 收到 rowid（索引树没有）和负载，返回 false 就停。
    fn walk(
        &self,
        n: u32,
        seen: &mut HashSet<u32>,
        depth: usize,
        opts: &Options<'_>,
        visit: &mut dyn FnMut(Option<i64>, Vec<u8>) -> bool,
    ) -> Result<bool, Error> {
        if depth > MAX_DEPTH || !seen.insert(n) {
            return Ok(true);
        }
        check_deadline(opts)?;
        let Some(page) = self.page(n) else {
            return Ok(true);
        };
        let hdr = if n == 1 { 100 } else { 0 };
        let Some(&kind) = page.get(hdr) else {
            return Ok(true);
        };
        let cells = be16(page, hdr + 3).unwrap_or(0);
        let interior = matches!(kind, 2 | 5);
        if !matches!(kind, 2 | 5 | 10 | 13) {
            return Ok(true);
        }
        let ptrs = hdr + if interior { 12 } else { 8 };
        for i in 0..cells {
            let Some(at) = be16(page, ptrs + i * 2) else {
                break;
            };
            match kind {
                13 => {
                    let Some((size, p)) = varint(page, at) else {
                        continue;
                    };
                    let Some((rowid, p)) = varint(page, p) else {
                        continue;
                    };
                    if let Some(pl) = self.payload(page, p, size as usize, false)
                        && !visit(Some(rowid as i64), pl)
                    {
                        return Ok(false);
                    }
                }
                5 => {
                    let Some(child) = be32(page, at) else {
                        continue;
                    };
                    if !self.walk(child, seen, depth + 1, opts, visit)? {
                        return Ok(false);
                    }
                }
                10 => {
                    let Some((size, p)) = varint(page, at) else {
                        continue;
                    };
                    if let Some(pl) = self.payload(page, p, size as usize, true)
                        && !visit(None, pl)
                    {
                        return Ok(false);
                    }
                }
                _ => {
                    let Some(child) = be32(page, at) else {
                        continue;
                    };
                    if !self.walk(child, seen, depth + 1, opts, visit)? {
                        return Ok(false);
                    }
                    let Some((size, p)) = varint(page, at + 4) else {
                        continue;
                    };
                    if let Some(pl) = self.payload(page, p, size as usize, true)
                        && !visit(None, pl)
                    {
                        return Ok(false);
                    }
                }
            }
        }
        if interior && let Some(right) = be32(page, hdr + 8) {
            return self.walk(right, seen, depth + 1, opts, visit);
        }
        Ok(true)
    }

    fn text(&self, raw: &[u8]) -> String {
        match self.encoding {
            0 => String::from_utf8_lossy(raw).into_owned(),
            e => {
                let units: Vec<u16> = raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| {
                        if e == 1 {
                            u16::from_le_bytes([c[0], c[1]])
                        } else {
                            u16::from_be_bytes([c[0], c[1]])
                        }
                    })
                    .collect();
                String::from_utf16_lossy(&units)
            }
        }
    }

    /// 解一条记录：头是一串序列类型，后面按类型依次是值。
    fn record(&self, pl: &[u8]) -> Vec<Value> {
        let mut out = Vec::new();
        let Some((hsize, mut p)) = varint(pl, 0) else {
            return out;
        };
        let hsize = hsize as usize;
        let mut body = hsize;
        while p < hsize {
            let Some((t, q)) = varint(pl, p) else { break };
            p = q;
            let (v, len) = match t {
                0 => (Value::Null, 0),
                1..=6 => {
                    let n = [1usize, 2, 3, 4, 6, 8][t as usize - 1];
                    let Some(raw) = pl.get(body..body + n) else {
                        break;
                    };
                    let mut x = if raw[0] & 0x80 != 0 { -1i64 } else { 0 };
                    for &b in raw {
                        x = x << 8 | i64::from(b);
                    }
                    (Value::Int(x), n)
                }
                7 => {
                    let Some(raw) = pl.get(body..body + 8) else {
                        break;
                    };
                    (
                        Value::Real(f64::from_be_bytes(raw.try_into().unwrap_or([0; 8]))),
                        8,
                    )
                }
                8 => (Value::Int(0), 0),
                9 => (Value::Int(1), 0),
                10 | 11 => (Value::Null, 0),
                t if t % 2 == 0 => {
                    let n = ((t - 12) / 2) as usize;
                    (Value::Blob(n), n)
                }
                t => {
                    let n = ((t - 13) / 2) as usize;
                    let Some(raw) = pl.get(body..body.saturating_add(n)) else {
                        break;
                    };
                    (Value::Text(self.text(raw)), n)
                }
            };
            out.push(v);
            body = body.saturating_add(len);
        }
        out
    }
}

/// 建表语句里的列信息。
#[derive(Debug, Default, PartialEq)]
struct Columns {
    names: Vec<String>,
    /// 存在记录里是 NULL、值等于 rowid 的列。
    rowid_alias: Option<usize>,
    /// WITHOUT ROWID 表的主键列：记录里排在最前。
    without_rowid_key: Option<Vec<usize>>,
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() >= 2 {
        let (a, z) = (b[0], b[b.len() - 1]);
        if (a == b'"' && z == b'"')
            || (a == b'`' && z == b'`')
            || (a == b'[' && z == b']')
            || (a == b'\'' && z == b'\'')
        {
            return s[1..s.len() - 1].to_owned();
        }
    }
    s.to_owned()
}

/// 按顶层逗号切，括号和引号里的不算。
fn split_top(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut quote, mut start) = (0usize, None::<char>, 0usize);
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some('['), ']') => quote = None,
            (Some(q), _) if q == c => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'' | '`' | '[') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

fn first_word(s: &str) -> String {
    let s = s.trim_start();
    let b = s.as_bytes();
    let close = match b.first() {
        Some(b'"') => Some('"'),
        Some(b'`') => Some('`'),
        Some(b'[') => Some(']'),
        Some(b'\'') => Some('\''),
        _ => None,
    };
    if let Some(c) = close
        && let Some(end) = s[1..].find(c)
    {
        return s[..end + 2].to_owned();
    }
    s.split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("")
        .to_owned()
}

fn parse_columns(sql: &str) -> Columns {
    let mut out = Columns::default();
    let Some(open) = sql.find('(') else {
        return out;
    };
    // 从左括号开始找配对的右括号
    let body = &sql[open + 1..];
    let (mut depth, mut quote, mut close) = (1usize, None::<char>, None);
    for (i, c) in body.char_indices() {
        match (quote, c) {
            (Some('['), ']') => quote = None,
            (Some(q), _) if q == c => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'' | '`' | '[') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else { return out };
    let tail = body[close + 1..].to_ascii_uppercase();
    let without_rowid = tail.contains("WITHOUT") && tail.contains("ROWID");
    let mut key: Vec<String> = Vec::new();
    let mut types: Vec<String> = Vec::new();
    for part in split_top(&body[..close]) {
        let part = part.trim();
        let word = first_word(part);
        let upper = word.to_ascii_uppercase();
        if matches!(
            upper.as_str(),
            "CONSTRAINT" | "PRIMARY" | "UNIQUE" | "CHECK" | "FOREIGN"
        ) {
            let up = part.to_ascii_uppercase();
            if let Some(i) = up.find("PRIMARY")
                && let Some(a) = part[i..].find('(')
                && let Some(b) = part[i + a..].find(')')
            {
                for k in split_top(&part[i + a + 1..i + a + b]) {
                    key.push(unquote(&first_word(k)));
                }
            }
            continue;
        }
        let rest = part[word.len()..].to_ascii_uppercase();
        let col = unquote(&word);
        if rest.contains("PRIMARY KEY") {
            key.push(col.clone());
        }
        types.push(rest.split_whitespace().next().unwrap_or("").to_owned());
        out.names.push(col);
    }
    let positions: Vec<usize> = key
        .iter()
        .filter_map(|k| out.names.iter().position(|n| n.eq_ignore_ascii_case(k)))
        .collect();
    if without_rowid {
        out.without_rowid_key = Some(positions);
    } else if let [only] = positions[..]
        && types[only] == "INTEGER"
    {
        out.rowid_alias = Some(only);
    }
    out
}

pub(crate) fn convert(bytes: &[u8], opts: &Options<'_>) -> Result<Converted, Error> {
    let db = Db::open(bytes)?;
    let mut tables: Vec<(String, u32, String)> = Vec::new();
    db.walk(1, &mut HashSet::new(), 0, opts, &mut |_, pl| {
        let row = db.record(&pl);
        if let [
            Value::Text(kind),
            Value::Text(name),
            _,
            Value::Int(root),
            sql,
            ..,
        ] = &row[..]
            && kind == "table"
            && *root > 0
            && !name.starts_with("sqlite_")
            && let Value::Text(sql) = sql
        {
            tables.push((name.clone(), *root as u32, sql.clone()));
        }
        true
    })?;
    let mut blocks = Vec::new();
    let mut warnings = Vec::new();
    let mut cells = 0usize;
    let max_cells = opts.limits.max_cells;
    for (name, root, sql) in &tables {
        check_deadline(opts)?;
        blocks.push(Block::Heading(2, md::escape(name)));
        let cols = parse_columns(sql);
        let mut rows = Vec::new();
        if !cols.names.is_empty() {
            rows.push(cols.names.iter().map(|n| md::escape(n)).collect::<Vec<_>>());
        }
        let width = cols.names.len();
        // WITHOUT ROWID 的记录是主键在前：算出记录位置到列位置的映射
        let order: Option<Vec<usize>> = cols.without_rowid_key.as_ref().map(|key| {
            let mut o = key.clone();
            o.extend((0..width).filter(|i| !key.contains(i)));
            o
        });
        let mut over = false;
        db.walk(*root, &mut HashSet::new(), 0, opts, &mut |rowid, pl| {
            let vals = db.record(&pl);
            if cells + width.max(vals.len()) > max_cells || budget::exhausted() {
                over = cells + width.max(vals.len()) > max_cells;
                return false;
            }
            let mut row = vec![String::new(); width.max(vals.len())];
            for (i, v) in vals.iter().enumerate() {
                let col = order.as_ref().and_then(|o| o.get(i)).copied().unwrap_or(i);
                let text = match (v, cols.rowid_alias, rowid) {
                    (Value::Null, Some(a), Some(id)) if a == col => id.to_string(),
                    _ => v.display(),
                };
                budget::spend(text.len());
                if let Some(slot) = row.get_mut(col) {
                    *slot = md::escape(&md::squeeze(&text));
                }
            }
            cells += row.len();
            rows.push(row);
            true
        })?;
        if over {
            warnings.push(format!("cells_truncated: stopped at {max_cells} cells"));
        }
        let longest = rows.iter().map(Vec::len).max().unwrap_or(0);
        for r in &mut rows {
            r.resize(longest, String::new());
        }
        if !rows.is_empty() {
            blocks.push(Block::Table(rows));
        }
        if over || budget::exhausted() {
            break;
        }
    }
    Ok(Converted {
        markdown: if blocks.is_empty() {
            String::new()
        } else {
            md::render(&blocks)
        },
        warnings,
        parts: tables.len(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests;
