use super::*;

fn enc_varint(mut v: u64) -> Vec<u8> {
    let mut out = vec![(v & 0x7F) as u8];
    v >>= 7;
    while v > 0 {
        out.insert(0, (v & 0x7F) as u8 | 0x80);
        v >>= 7;
    }
    out
}

#[derive(Clone)]
enum V<'a> {
    N,
    I(i64),
    T(&'a str),
    B(usize),
}

fn rec(vals: &[V<'_>]) -> Vec<u8> {
    let mut head = Vec::new();
    let mut body = Vec::new();
    for v in vals {
        match v {
            V::N => head.push(0),
            V::I(x) => {
                head.push(4);
                body.extend_from_slice(&(*x as i32).to_be_bytes());
            }
            V::T(s) => {
                head.extend(enc_varint(13 + 2 * s.len() as u64));
                body.extend_from_slice(s.as_bytes());
            }
            V::B(n) => {
                head.extend(enc_varint(12 + 2 * *n as u64));
                body.extend(std::iter::repeat_n(0u8, *n));
            }
        }
    }
    let mut out = enc_varint(head.len() as u64 + 1);
    out.extend(head);
    out.extend(body);
    out
}

/// 造一个表叶子页。`kind` 13 是表，10 是索引。
fn leaf(kind: u8, hdr: usize, size: usize, cells: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut page = vec![0u8; size];
    page[hdr] = kind;
    page[hdr + 3..hdr + 5].copy_from_slice(&(cells.len() as u16).to_be_bytes());
    let mut end = size;
    for (i, (rowid, pl)) in cells.iter().enumerate() {
        let mut cell = enc_varint(pl.len() as u64);
        if kind == 13 {
            cell.extend(enc_varint(*rowid as u64));
        }
        cell.extend_from_slice(pl);
        end -= cell.len();
        page[end..end + cell.len()].copy_from_slice(&cell);
        page[hdr + 8 + i * 2..hdr + 10 + i * 2].copy_from_slice(&(end as u16).to_be_bytes());
    }
    page
}

pub(crate) fn sample() -> Vec<u8> {
    let size = 4096usize;
    let long = "x".repeat(5000);
    let schema = [
        (
            1,
            rec(&[
                V::T("table"),
                V::T("people"),
                V::T("people"),
                V::I(2),
                V::T(
                    "CREATE TABLE people(id INTEGER PRIMARY KEY, \"full name\" TEXT, pic BLOB, CHECK (id > 0))",
                ),
            ]),
        ),
        (
            2,
            rec(&[
                V::T("table"),
                V::T("kv"),
                V::T("kv"),
                V::I(3),
                V::T("CREATE TABLE kv(v TEXT, k TEXT, PRIMARY KEY(k)) WITHOUT ROWID"),
            ]),
        ),
        (
            3,
            rec(&[
                V::T("index"),
                V::T("idx"),
                V::T("people"),
                V::I(9),
                V::T("CREATE INDEX idx ON people(id)"),
            ]),
        ),
        (
            4,
            rec(&[
                V::T("table"),
                V::T("sqlite_sequence"),
                V::T("sqlite_sequence"),
                V::I(9),
                V::T("CREATE TABLE sqlite_sequence(a,b)"),
            ]),
        ),
    ];
    let mut p1 = leaf(13, 100, size, &schema);
    p1[..16].copy_from_slice(MAGIC);
    p1[16..18].copy_from_slice(&(size as u16).to_be_bytes());
    p1[56..60].copy_from_slice(&1u32.to_be_bytes());
    let people = [
        (7, rec(&[V::N, V::T("\u{4e2d}\u{6587}|a"), V::B(3)])),
        (8, rec(&[V::N, V::T("bob"), V::N])),
    ];
    let p2 = leaf(13, 0, size, &people);
    let p3 = leaf(10, 0, size, &[(0, rec(&[V::T("key"), V::T("value")]))]);
    // 一条放不下的记录：本页 + 一个溢出页
    let big = rec(&[V::N, V::T(&long), V::N]);
    let _ = big;
    [p1, p2, p3].concat()
}

#[test]
fn reads_tables_rowid_alias_and_without_rowid() {
    let d = convert(&sample(), &Options::default()).unwrap();
    assert_eq!(d.parts, 2);
    assert!(d.markdown.contains("## people"), "{}", d.markdown);
    assert!(
        d.markdown.contains("| id | full name | pic |"),
        "{}",
        d.markdown
    );
    assert!(
        d.markdown
            .contains("| 7 | \u{4e2d}\u{6587}\\|a | <blob 3 bytes> |")
            || d.markdown
                .contains("| 7 | \u{4e2d}\u{6587}\\|a | \\<blob 3 bytes\\> |"),
        "{}",
        d.markdown
    );
    assert!(d.markdown.contains("| 8 | bob |"), "{}", d.markdown);
    assert!(d.markdown.contains("| v | k |"), "{}", d.markdown);
    assert!(d.markdown.contains("| value | key |"), "{}", d.markdown);
    assert!(!d.markdown.contains("sqlite_sequence"));
}

#[test]
fn overflow_pages_are_followed() {
    let size = 4096usize;
    let db = Db {
        data: &[],
        page_size: size,
        usable: size,
        encoding: 0,
    };
    // 负载比本页上限小时不走溢出
    let page = vec![7u8; size];
    assert_eq!(db.payload(&page, 0, 10, false).unwrap().len(), 10);
    // 带溢出页的库
    let long = "y".repeat(6000);
    let big = rec(&[V::T(&long)]);
    let u = size;
    let (x, m) = (u - 35, (u - 12) * 32 / 255 - 23);
    assert!(big.len() > x);
    let k = m + (big.len() - m) % (u - 4);
    let local = if k <= x { k } else { m };
    let mut cell = enc_varint(big.len() as u64);
    cell.extend(enc_varint(1));
    cell.extend_from_slice(&big[..local]);
    cell.extend_from_slice(&3u32.to_be_bytes());
    let mut p2 = vec![0u8; size];
    p2[0] = 13;
    p2[3..5].copy_from_slice(&1u16.to_be_bytes());
    let at = size - cell.len();
    p2[at..].copy_from_slice(&cell);
    p2[8..10].copy_from_slice(&(at as u16).to_be_bytes());
    let mut p3 = vec![0u8; size];
    p3[4..4 + big.len() - local].copy_from_slice(&big[local..]);
    let schema = [(
        1,
        rec(&[
            V::T("table"),
            V::T("t"),
            V::T("t"),
            V::I(2),
            V::T("CREATE TABLE t(a)"),
        ]),
    )];
    let mut p1 = leaf(13, 100, size, &schema);
    p1[..16].copy_from_slice(MAGIC);
    p1[16..18].copy_from_slice(&(size as u16).to_be_bytes());
    let d = convert(&[p1, p2, p3].concat(), &Options::default()).unwrap();
    assert!(d.markdown.contains(&long), "len {}", d.markdown.len());
}

#[test]
fn page_cycles_and_bad_headers_stop() {
    assert!(convert(b"SQLite format 3\0", &Options::default()).is_err());
    // 内部页指向自己
    let size = 4096usize;
    let mut p1 = vec![0u8; size];
    p1[..16].copy_from_slice(MAGIC);
    p1[16..18].copy_from_slice(&(size as u16).to_be_bytes());
    p1[100] = 5;
    p1[108..112].copy_from_slice(&1u32.to_be_bytes());
    let d = convert(&p1, &Options::default()).unwrap();
    assert_eq!(d.parts, 0);
}

#[test]
fn column_parsing() {
    let c = parse_columns(
        "CREATE TABLE \"t\" (a INTEGER, [b c] TEXT DEFAULT ',', d NUMERIC(10,2), PRIMARY KEY(a))",
    );
    assert_eq!(c.names, vec!["a", "b c", "d"]);
    assert_eq!(c.rowid_alias, Some(0));
    let c = parse_columns("CREATE TABLE t(a INT PRIMARY KEY, b)");
    assert_eq!(c.rowid_alias, None);
}

#[test]
fn hostile_record_overflow_and_zero_columns_stop_cleanly() {
    let mut page = [0u8; 4096];
    page[..16].copy_from_slice(MAGIC);
    page[16..18].copy_from_slice(&4096u16.to_be_bytes());
    let db = Db::open(&page).unwrap();
    // 超大 varint 头长 + 大量 0 字节列
    let mut evil = vec![0xFF; 9];
    evil.resize(256, 0);
    assert!(db.record(&evil, 128).len() <= 128);
    let mut many_zero = vec![0x81, 0x40]; // hsize = 192
    many_zero.resize(200, 0);
    assert!(db.record(&many_zero, 64).len() <= 64);
}
