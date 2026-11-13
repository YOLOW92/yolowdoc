//! 内部热点归因微基准（含分配次数、累计分配字节和存活峰值统计）。
//! 通过 `cargo test --release perf_ -- --ignored --nocapture` 运行。

use std::hint::black_box;
use std::time::Instant;

use crate::{Format, Limits, Options, convert, md, test_probe, xml, zip};

fn bench_op<T>(name: &str, iters: usize, bytes_per_iter: usize, mut f: impl FnMut() -> T) {
    let (_, st) = test_probe::measure(|| black_box(f()));
    let mut times = Vec::with_capacity(15);
    for _ in 0..15 {
        let t0 = Instant::now();
        for _ in 0..iters {
            black_box(f());
        }
        times.push(t0.elapsed().as_secs_f64() * 1e6 / iters as f64);
    }
    times.sort_by(f64::total_cmp);
    let p50 = times[times.len() / 2];
    let p95 = times[(times.len() * 95 / 100).min(times.len() - 1)];
    let mb_s = if bytes_per_iter > 0 && p50 > 0.0 {
        (bytes_per_iter as f64 / (1024.0 * 1024.0)) / (p50 / 1e6)
    } else {
        0.0
    };
    eprintln!(
        "{name:<28} p50={p50:>8.3} us  p95={p95:>8.3} us  {mb_s:>8.2} MiB/s | allocs={:>7}  total={:>9} B  peak={:>9} B",
        st.allocs, st.total_bytes, st.peak
    );
}

#[test]
#[ignore = "hotspot attribution microbenchmark; run with --ignored --nocapture"]
fn perf_micro_attribution() {
    // 1. ZIP lookup (500 entries)
    let names: Vec<String> = (0..500)
        .map(|i| format!("xl/worksheets/sheet{i:03}.xml"))
        .chain(["Übung/Über.txt".to_owned()])
        .collect();
    let specs: Vec<(&str, &[u8], bool)> = names
        .iter()
        .map(|n| (n.as_str(), &b"x"[..], false))
        .collect();
    let zbytes = zip::tests::build(&specs);
    let z = zip::Zip::open(&zbytes, &Limits::default()).unwrap();
    bench_op("zip_lookup_hit_lower", 1000, 0, || {
        z.contains("xl/worksheets/sheet250.xml")
    });
    bench_op("zip_lookup_hit_slash", 1000, 0, || {
        z.contains("/xl/worksheets/sheet250.xml")
    });
    bench_op("zip_lookup_miss_ascii", 1000, 0, || {
        z.contains("xl/worksheets/missing.xml")
    });
    bench_op("zip_lookup_hit_upper", 1000, 0, || {
        z.contains("XL/WORKSHEETS/SHEET250.XML")
    });
    bench_op("zip_lookup_hit_unicode", 1000, 0, || {
        z.contains("Übung/Über.txt")
    });

    // 2. XML unescape + scan
    let short_plain = "Simple cell value 123.45";
    let long_plain = "Regular paragraph payload without entities ".repeat(2000);
    let dense_ent = "&lt;div class=&quot;a&amp;b&quot;&gt;&#65;&#x1F600;&apos; ".repeat(200);
    bench_op("xml_unescape_short_plain", 5000, short_plain.len(), || {
        xml::unescape(short_plain).len()
    });
    bench_op("xml_unescape_long_plain", 200, long_plain.len(), || {
        xml::unescape(&long_plain).len()
    });
    bench_op("xml_unescape_dense_ent", 500, dense_ent.len(), || {
        xml::unescape(&dense_ent).len()
    });

    let mut xml_doc = String::from("<worksheet><sheetData>");
    for r in 1..=2000 {
        xml_doc.push_str(&format!(
            r#"<row r="{r}"><c r="A{r}" t="s" s="1"><v>{r}</v></c><c r="B{r}" s="2"><v>{r}.5</v></c></row>"#
        ));
    }
    xml_doc.push_str("</sheetData></worksheet>");
    bench_op("xml_reader_scan_2000r", 20, xml_doc.len(), || {
        let mut r = xml::Reader::new(&xml_doc);
        let mut n = 0usize;
        while let Ok(Some(t)) = r.next() {
            if let xml::Tok::Start(_, a) = t {
                for (k, v) in a.raw() {
                    n += k.len() + v.len();
                }
            }
        }
        n
    });

    // 3. XLSX 4000x8 phase attribution + shared-string repeated workload
    let mut ss = String::from("<sst>");
    for r in 0..500 {
        ss.push_str(&format!("<si><t>Item-{r:04}-payload</t></si>"));
    }
    ss.push_str("</sst>");
    let mut sheet = String::from("<worksheet><sheetData>");
    for r in 1..=4000 {
        sheet.push_str(&format!(r#"<row r="{r}">"#));
        for c in 0..8 {
            let col = (b'A' + (c as u8)) as char;
            if c == 0 {
                sheet.push_str(&format!(
                    r#"<c r="{col}{r}" t="s"><v>{}</v></c>"#,
                    (r - 1) % 500
                ));
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
    let xlsx_bytes = zip::tests::build(&[
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets></workbook>"#, true),
        ("xl/_rels/workbook.xml.rels", br#"<Relationships><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#, true),
        ("xl/sharedStrings.xml", ss.as_bytes(), true),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes(), true),
    ]);
    let opts = Options::default();
    bench_op("xlsx_4000x8_full", 5, xlsx_bytes.len(), || {
        convert(&xlsx_bytes, Format::Xlsx, &opts)
            .unwrap()
            .markdown
            .len()
    });
    bench_op("xlsx_4000x8_zip_open_read", 10, xlsx_bytes.len(), || {
        let z = zip::Zip::open(&xlsx_bytes, &opts.limits).unwrap();
        z.read_text("xl/worksheets/sheet1.xml")
            .unwrap()
            .unwrap()
            .len()
    });
    let z = zip::Zip::open(&xlsx_bytes, &opts.limits).unwrap();
    let _sx = z.read_text("xl/worksheets/sheet1.xml").unwrap().unwrap();
    bench_op("md_render_heading", 1000, 0, || {
        md::render(&[md::Block::Heading(2, "Data".into())]).len()
    });

    // Repeated long shared string workload (5000 cells x 512 B shared string = ~2.5 MiB)
    let long_s = "SharedStringValue-".repeat(28);
    let shared_long = format!("<sst><si><t>{long_s}</t></si></sst>");
    let mut rep_sheet = String::from("<worksheet><sheetData>");
    for r in 1..=5000 {
        rep_sheet.push_str(&format!(
            r#"<row r="{r}"><c r="A{r}" t="s"><v>0</v></c></row>"#
        ));
    }
    rep_sheet.push_str("</sheetData></worksheet>");
    let rep_xlsx = zip::tests::build(&[
        ("xl/workbook.xml", br#"<workbook><sheets><sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#, true),
        ("xl/_rels/workbook.xml.rels", br#"<Relationships><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#, true),
        ("xl/sharedStrings.xml", shared_long.as_bytes(), true),
        ("xl/worksheets/sheet1.xml", rep_sheet.as_bytes(), true),
    ]);
    bench_op("xlsx_5000_rep_shared_512B", 5, rep_xlsx.len(), || {
        convert(&rep_xlsx, Format::Xlsx, &opts)
            .unwrap()
            .markdown
            .len()
    });
}
