use std::hint::black_box;
use std::time::Instant;

use yolowdoc::{Format, Options, convert, media};

fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn build_zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for (name, data, deflate) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let (method, body): (u16, Vec<u8>) = if *deflate {
            (8, miniz_oxide::deflate::compress_to_vec(data, 6))
        } else {
            (0, data.to_vec())
        };
        let head = |sig: u32, central: bool| {
            let mut h = sig.to_le_bytes().to_vec();
            if central {
                h.extend_from_slice(&[20, 0]);
            }
            h.extend_from_slice(&[20, 0, 0, 0]);
            h.extend_from_slice(&method.to_le_bytes());
            h.extend_from_slice(&[0, 0, 0, 0]);
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(body.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&[0, 0]);
            if central {
                h.extend_from_slice(&[0; 10]);
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(head(0x0403_4b50, false));
        out.extend_from_slice(&body);
        cd.extend(head(0x0201_4b50, true));
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out
}

fn make_docx(paragraphs: usize) -> Vec<u8> {
    let mut doc = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>"#,
    );
    for i in 0..paragraphs {
        if i % 25 == 0 {
            doc.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Section {}</w:t></w:r></w:p>"#,
                i / 25 + 1
            ));
        } else {
            doc.push_str(&format!(
                r#"<w:p><w:r><w:t>Paragraph {i}: benchmark payload with some longer prose text to exercise XML text scanning and run merging across multiple sentences.</w:t></w:r></w:p>"#
            ));
        }
    }
    doc.push_str(r#"<w:p><w:r><w:drawing><a:blip r:embed="img1"/></w:drawing></w:r></w:p>"#);
    doc.push_str("</w:body></w:document>");
    let styles = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#;
    let rels = r#"<Relationships><Relationship Id="s" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="img1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/chart.png"/></Relationships>"#;
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend(std::iter::repeat_n(0xAB, 32 << 10));
    build_zip(&[
        ("word/document.xml", doc.as_bytes(), true),
        ("word/styles.xml", styles.as_bytes(), true),
        ("word/_rels/document.xml.rels", rels.as_bytes(), true),
        ("word/media/chart.png", &png, false),
    ])
}

fn make_xlsx(rows: usize, cols: usize) -> Vec<u8> {
    let mut ss = String::from(
        r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    );
    for r in 0..rows.min(500) {
        ss.push_str(&format!("<si><t>Item-{r:04}-payload</t></si>"));
    }
    ss.push_str("</sst>");
    let mut sheet = String::from(
        r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    for r in 1..=rows {
        sheet.push_str(&format!(r#"<row r="{r}">"#));
        for c in 0..cols {
            let col = (b'A' + (c as u8)) as char;
            if c == 0 {
                let idx = (r - 1) % 500;
                sheet.push_str(&format!(r#"<c r="{col}{r}" t="s"><v>{idx}</v></c>"#));
            } else {
                sheet.push_str(&format!(
                    r#"<c r="{col}{r}"><v>{:.2}</v></c>"#,
                    (r * (c + 1)) as f64 * 0.25
                ));
            }
        }
        sheet.push_str("</row>");
    }
    sheet.push_str("</sheetData></worksheet>");
    let wb = r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    let wrels = r#"<Relationships><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#;
    build_zip(&[
        ("xl/workbook.xml", wb.as_bytes(), true),
        ("xl/_rels/workbook.xml.rels", wrels.as_bytes(), true),
        ("xl/sharedStrings.xml", ss.as_bytes(), true),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes(), true),
    ])
}

fn make_pptx(slides: usize) -> Vec<u8> {
    let mut pres = String::from(
        r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst>"#,
    );
    let mut prels = String::from("<Relationships>");
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 1..=slides {
        pres.push_str(&format!(r#"<p:sldId id="{}" r:id="s{i}"/>"#, 255 + i));
        prels.push_str(&format!(
            r#"<Relationship Id="s{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>"#
        ));
        let slide = format!(
            r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="1" name="Title"/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Slide {i}</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:txBody><a:p><a:r><a:t>Bullet item A on slide {i}</a:t></a:r></a:p><a:p><a:r><a:t>Bullet item B on slide {i}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
        );
        entries.push((format!("ppt/slides/slide{i}.xml"), slide.into_bytes()));
    }
    pres.push_str("</p:sldIdLst></p:presentation>");
    prels.push_str("</Relationships>");
    entries.push(("ppt/presentation.xml".into(), pres.into_bytes()));
    entries.push(("ppt/_rels/presentation.xml.rels".into(), prels.into_bytes()));
    let refs: Vec<(&str, &[u8], bool)> = entries
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice(), true))
        .collect();
    build_zip(&refs)
}

fn make_media_docx(images: usize) -> Vec<u8> {
    let mut doc = String::from(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>"#,
    );
    let mut rels = String::from("<Relationships>");
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..images {
        doc.push_str(&format!(
            r#"<w:p><w:r><w:drawing><a:blip r:embed="r{i}"/></w:drawing></w:r></w:p>"#
        ));
        rels.push_str(&format!(
            r#"<Relationship Id="r{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img{i}.png"/>"#
        ));
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend(std::iter::repeat_n((i & 0xff) as u8, 16 << 10));
        files.push((format!("word/media/img{i}.png"), png));
    }
    doc.push_str("</w:body></w:document>");
    rels.push_str("</Relationships>");
    files.push(("word/document.xml".into(), doc.into_bytes()));
    files.push(("word/_rels/document.xml.rels".into(), rels.into_bytes()));
    let refs: Vec<(&str, &[u8], bool)> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice(), false))
        .collect();
    build_zip(&refs)
}

fn make_csv(rows: usize) -> Vec<u8> {
    let mut out = String::from("id,name,region,amount,note\n");
    for i in 0..rows {
        out.push_str(&format!(
            "{i},\"Product, #{i}\",ap-northeast-1,{:.2},regular item\n",
            i as f64 * 1.5
        ));
    }
    out.into_bytes()
}

fn make_tar(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data) in files {
        let mut hdr = [0u8; 512];
        let nb = name.as_bytes();
        let copy_n = nb.len().min(100);
        hdr[..copy_n].copy_from_slice(&nb[..copy_n]);
        hdr[100..108].copy_from_slice(b"0000644\0");
        hdr[108..116].copy_from_slice(b"0000000\0");
        hdr[116..124].copy_from_slice(b"0000000\0");
        let sz = format!("{:011o}\0", data.len());
        hdr[124..136].copy_from_slice(sz.as_bytes());
        hdr[136..148].copy_from_slice(b"00000000000\0");
        hdr[148..156].copy_from_slice(b"        ");
        hdr[156] = b'0';
        hdr[257..263].copy_from_slice(b"ustar\0");
        hdr[263..265].copy_from_slice(b"00");
        let sum: u32 = hdr.iter().map(|&b| u32::from(b)).sum();
        let cksum = format!("{sum:06o}\0 ");
        hdr[148..156].copy_from_slice(cksum.as_bytes());
        out.extend_from_slice(&hdr);
        out.extend_from_slice(data);
        let pad = (512 - (data.len() % 512)) % 512;
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    out.extend(std::iter::repeat_n(0u8, 1024));
    out
}

fn enc_varint(mut v: u64) -> Vec<u8> {
    let mut tmp = [0u8; 9];
    let mut n = 0usize;
    loop {
        tmp[n] = (v & 0x7F) as u8;
        n += 1;
        v >>= 7;
        if v == 0 {
            break;
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in (0..n).rev() {
        let hi = if i > 0 { 0x80 } else { 0 };
        out.push(tmp[i] | hi);
    }
    out
}

fn sqlite_leaf(hdr: usize, page_size: usize, cells: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut page = vec![0u8; page_size];
    page[hdr] = 13;
    page[hdr + 3..hdr + 5].copy_from_slice(&(cells.len() as u16).to_be_bytes());
    let mut end = page_size;
    for (i, (rowid, pl)) in cells.iter().enumerate() {
        let mut cell = enc_varint(pl.len() as u64);
        cell.extend(enc_varint(*rowid as u64));
        cell.extend_from_slice(pl);
        end -= cell.len();
        page[end..end + cell.len()].copy_from_slice(&cell);
        page[hdr + 8 + i * 2..hdr + 10 + i * 2].copy_from_slice(&(end as u16).to_be_bytes());
    }
    page
}

fn make_sqlite_btree(rows_per_leaf: usize, num_leaves: usize) -> Vec<u8> {
    const SZ: usize = 4096;
    fn text_rec(cols: &[&str]) -> Vec<u8> {
        let mut h = Vec::new();
        let mut b = Vec::new();
        for s in cols {
            h.extend(enc_varint(13 + 2 * s.len() as u64));
            b.extend_from_slice(s.as_bytes());
        }
        let mut out = enc_varint(h.len() as u64 + 1);
        out.extend(h);
        out.extend(b);
        out
    }
    fn schema_rec(tbl: &str, root: i64, sql: &str) -> Vec<u8> {
        let t = b"table";
        let sql_b = sql.as_bytes();
        let tb = tbl.as_bytes();
        let mut h = vec![
            (13 + 2 * t.len()) as u8,
            (13 + 2 * tb.len()) as u8,
            (13 + 2 * tb.len()) as u8,
            4u8,
        ];
        h.extend(enc_varint(13 + 2 * sql_b.len() as u64));
        let mut out = enc_varint(h.len() as u64 + 1);
        out.extend(h);
        out.extend_from_slice(t);
        out.extend_from_slice(tb);
        out.extend_from_slice(tb);
        out.extend_from_slice(&(root as i32).to_be_bytes());
        out.extend_from_slice(sql_b);
        out
    }

    let schema_cell = schema_rec(
        "items",
        2,
        "CREATE TABLE items(name TEXT, region TEXT, payload TEXT)",
    );
    let mut p1 = sqlite_leaf(100, SZ, &[(1, schema_cell)]);
    p1[..16].copy_from_slice(b"SQLite format 3\0");
    p1[16..18].copy_from_slice(&(SZ as u16).to_be_bytes());
    p1[18] = 1;
    p1[19] = 1;

    // Page 2 = interior table B-tree page pointing to leaves 3..=2+num_leaves
    let mut interior = vec![0u8; SZ];
    interior[0] = 5;
    let n_children = num_leaves - 1;
    interior[3..5].copy_from_slice(&(n_children as u16).to_be_bytes());
    interior[8..12].copy_from_slice(&((2 + num_leaves) as u32).to_be_bytes());
    let mut end = SZ;
    for idx in 0..n_children {
        let child_pg = (3 + idx) as u32;
        let max_key = ((idx + 1) * rows_per_leaf) as u64;
        let mut cell = Vec::with_capacity(12);
        cell.extend_from_slice(&child_pg.to_be_bytes());
        cell.extend(enc_varint(max_key));
        end -= cell.len();
        interior[end..end + cell.len()].copy_from_slice(&cell);
        interior[12 + idx * 2..14 + idx * 2].copy_from_slice(&(end as u16).to_be_bytes());
    }

    let mut db = Vec::with_capacity((2 + num_leaves) * SZ);
    db.extend_from_slice(&p1);
    db.extend_from_slice(&interior);
    for l_idx in 0..num_leaves {
        let mut cells = Vec::with_capacity(rows_per_leaf);
        for r in 0..rows_per_leaf {
            let id = (l_idx * rows_per_leaf + r + 1) as i64;
            let name = format!("item-{id:05}");
            cells.push((
                id,
                text_rec(&[&name, "ap-northeast-1", "payload-value-0123456789"]),
            ));
        }
        db.extend_from_slice(&sqlite_leaf(0, SZ, &cells));
    }
    db
}

fn make_xlsx_shared(rows: usize, str_len: usize) -> Vec<u8> {
    let payload = "S".repeat(str_len);
    let ss = format!("<sst><si><t>{payload}</t></si></sst>");
    let mut sheet = String::with_capacity(rows * 32 + 64);
    sheet.push_str("<worksheet><sheetData>");
    for r in 1..=rows {
        sheet.push_str(&format!(
            r#"<row r="{r}"><c r="A{r}" t="s"><v>0</v></c></row>"#
        ));
    }
    sheet.push_str("</sheetData></worksheet>");
    build_zip(&[
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="S1" sheetId="1" r:id="rId1"/></sheets></workbook>"#, false),
        ("xl/_rels/workbook.xml.rels", br#"<Relationships><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#, false),
        ("xl/sharedStrings.xml", ss.as_bytes(), false),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes(), false),
    ])
}

fn vm_hwm_kb() -> usize {
    let Ok(s) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

fn bench(name: &str, input_bytes: usize, rounds: usize, mut f: impl FnMut() -> usize) {
    let out_bytes = black_box(f());
    let mut times_ms = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let t0 = Instant::now();
        black_box(f());
        times_ms.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    times_ms.sort_by(f64::total_cmp);
    let p50 = times_ms[times_ms.len() / 2];
    let p95 = times_ms[(times_ms.len() * 95 / 100).min(times_ms.len() - 1)];
    let min = times_ms[0];
    let mb_s = (input_bytes as f64 / (1024.0 * 1024.0)) / (p50 / 1000.0);
    println!(
        "{name:<20} in={input_bytes:>9} B  out={out_bytes:>8}  p50={p50:>7.3} ms  p95={p95:>7.3} ms  min={min:>7.3} ms  {mb_s:>7.2} MiB/s  hwm={} KB",
        vm_hwm_kb()
    );
}

fn main() {
    let rounds = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);
    let heavy = rounds.clamp(3, 5);
    let opts = Options::default();

    let docx_s = make_docx(200);
    let docx_m = make_docx(2_500);
    let docx_l = make_docx(20_000);
    let xlsx_s = make_xlsx(200, 6);
    let xlsx_m = make_xlsx(4_000, 8);
    let xlsx_rep = make_xlsx_shared(5_000, 512);
    let xlsx_l = make_xlsx(25_000, 4);
    let pptx_m = make_pptx(60);
    let pptx_l = make_pptx(300);
    let media_doc = make_media_docx(40);
    let csv_m = make_csv(10_000);
    let csv_l = make_csv(1_000_000);
    let sq_m = make_sqlite_btree(40, 120);
    let arch_files: Vec<(String, Vec<u8>)> = (0..200)
        .map(|i| {
            (
                format!("dir/f{i:03}.txt"),
                format!("line {i} ok\n").into_bytes(),
            )
        })
        .collect();
    let arch_refs: Vec<(&str, &[u8], bool)> = arch_files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice(), false))
        .collect();
    let zip_arch = build_zip(&arch_refs);
    let tar_refs: Vec<(&str, &[u8])> = arch_files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let tar_arch = make_tar(&tar_refs);
    let nested_zip = build_zip(&[
        ("inner.zip", &zip_arch, false),
        ("inner.tar", &tar_arch, false),
    ]);

    println!("yolowdoc bench ({rounds} rounds / {heavy} heavy rounds)");
    bench("docx_200p", docx_s.len(), rounds, || {
        convert(&docx_s, Format::Docx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("docx_2500p", docx_m.len(), rounds, || {
        convert(&docx_m, Format::Docx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("docx_20000p", docx_l.len(), heavy, || {
        convert(&docx_l, Format::Docx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("xlsx_200x6", xlsx_s.len(), rounds, || {
        convert(&xlsx_s, Format::Xlsx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("xlsx_4000x8", xlsx_m.len(), rounds, || {
        convert(&xlsx_m, Format::Xlsx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("xlsx_5k_rep_512B", xlsx_rep.len(), rounds, || {
        convert(&xlsx_rep, Format::Xlsx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("xlsx_25000x4", xlsx_l.len(), heavy, || {
        convert(&xlsx_l, Format::Xlsx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("pptx_60s", pptx_m.len(), rounds, || {
        convert(&pptx_m, Format::Pptx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("pptx_300s", pptx_l.len(), heavy, || {
        convert(&pptx_l, Format::Pptx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("media_40img", media_doc.len(), rounds, || {
        media(&media_doc, Format::Docx, &opts).unwrap().len()
    });
    bench("csv_10000r", csv_m.len(), rounds, || {
        convert(&csv_m, Format::Csv, &opts).unwrap().markdown.len()
    });
    bench("csv_1M_rows", csv_l.len(), heavy, || {
        convert(&csv_l, Format::Csv, &opts).unwrap().markdown.len()
    });
    bench("sqlite_4800r", sq_m.len(), rounds, || {
        convert(&sq_m, Format::Sqlite, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("zip_200f", zip_arch.len(), rounds, || {
        convert(&zip_arch, Format::Archive, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench("tar_200f", tar_arch.len(), rounds, || {
        convert(&tar_arch, Format::Archive, &opts)
            .unwrap()
            .markdown
            .len()
    });
    let nested_member = |bytes: &[u8],
                         name: &str,
                         max_bytes: usize|
     -> Result<Option<yolowdoc::Converted>, yolowdoc::Error> {
        let Some(fmt) = Format::detect(bytes, Some(name), None) else {
            return Ok(None);
        };
        let mut inner = Options::default();
        inner.limits.max_output_bytes = max_bytes;
        convert(bytes, fmt, &inner).map(Some)
    };
    let nested_opts = Options {
        member: Some(&nested_member),
        ..Options::default()
    };
    bench("nested_archive_400f", nested_zip.len(), rounds, || {
        convert(&nested_zip, Format::Archive, &nested_opts)
            .unwrap()
            .markdown
            .len()
    });
}
