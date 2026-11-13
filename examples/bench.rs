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

fn bench<T>(name: &str, input_bytes: usize, rounds: usize, mut f: impl FnMut() -> T) {
    black_box(f());
    let mut times_ms = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let t0 = Instant::now();
        black_box(f());
        times_ms.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    times_ms.sort_by(f64::total_cmp);
    let med = times_ms[times_ms.len() / 2];
    let min = times_ms[0];
    let mb_s = (input_bytes as f64 / (1024.0 * 1024.0)) / (med / 1000.0);
    println!(
        "{name:<18} {input_bytes:>9} B   med {med:>7.3} ms   min {min:>7.3} ms   {mb_s:>7.2} MiB/s"
    );
}

fn main() {
    let rounds = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);
    let opts = Options::default();

    let docx = make_docx(2_500);
    let xlsx = make_xlsx(4_000, 8);
    let pptx = make_pptx(60);
    let media_doc = make_media_docx(40);
    let csv = make_csv(10_000);

    println!("yolowdoc bench ({rounds} rounds, median)");
    bench("docx_2500p", docx.len(), rounds, || {
        convert(&docx, Format::Docx, &opts).unwrap()
    });
    bench("xlsx_4000x8", xlsx.len(), rounds, || {
        convert(&xlsx, Format::Xlsx, &opts).unwrap()
    });
    bench("pptx_60s", pptx.len(), rounds, || {
        convert(&pptx, Format::Pptx, &opts).unwrap()
    });
    bench("media_40img", media_doc.len(), rounds, || {
        media(&media_doc, Format::Docx, &opts).unwrap()
    });
    bench("csv_10000r", csv.len(), rounds, || {
        convert(&csv, Format::Csv, &opts).unwrap()
    });
}
