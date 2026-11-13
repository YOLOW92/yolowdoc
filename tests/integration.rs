use std::thread;
use std::time::{Duration, Instant};

use yolowdoc::{
    Error, Format, Limits, Options, convert, convert_csv_text, convert_text, media, media_bytes,
};

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

fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for (name, data) in entries {
        let crc = crc32(data);
        let offset = out.len() as u32;
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cd.extend_from_slice(&[0; 12]);
        cd.extend_from_slice(&offset.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
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

fn minimal_docx(with_media: bool) -> Vec<u8> {
    let doc = br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Hello Title</w:t></w:r></w:p><w:p><w:r><w:t>Body text</w:t></w:r></w:p></w:body></w:document>"#;
    let styles = br#"<?xml version="1.0"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#;
    let doc_rels = br#"<Relationships><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    if !with_media {
        return build_zip(&[
            ("word/document.xml", doc),
            ("word/_rels/document.xml.rels", doc_rels),
            ("word/styles.xml", styles),
        ]);
    }
    let doc_img = br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body><w:p><w:r><w:drawing><a:blip r:embed="rId1"/></w:drawing></w:r></w:p></w:body></w:document>"#;
    let rels = br#"<Relationships><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/logo.png"/></Relationships>"#;
    let png = b"\x89PNG\r\n\x1a\nfake";
    build_zip(&[
        ("word/document.xml", doc_img),
        ("word/_rels/document.xml.rels", rels),
        ("word/media/logo.png", png),
    ])
}

fn minimal_epub() -> Vec<u8> {
    let container = br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
    let opf = br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:title>Book</dc:title><dc:creator>Ada</dc:creator></metadata><manifest><item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#;
    let ch1 = b"<html><body><h1>Ch 1</h1><p>Alpha</p></body></html>";
    build_zip(&[
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        ("OEBPS/ch1.xhtml", ch1),
    ])
}

#[test]
fn detect_and_basic_text_and_csv_workflows() {
    let md_utf16_le: Vec<u8> = [0xFFu8, 0xFE]
        .into_iter()
        .chain("# Top\n\nHi\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(
        Format::detect(&md_utf16_le, Some("doc.md"), None),
        Some(Format::Markdown)
    );
    let doc = convert(&md_utf16_le, Format::Markdown, &Options::default()).unwrap();
    assert_eq!(doc.title.as_deref(), Some("Top"));
    assert!(doc.markdown.contains("# Top\n\nHi\n"));

    let csv_doc = convert_csv_text("col1,col2\n1,2\n", &Options::default()).unwrap();
    assert_eq!(
        csv_doc.markdown,
        "| col1 | col2 |\n| --- | --- |\n| 1 | 2 |\n"
    );
    assert_eq!(csv_doc.parts, 2);

    let json_doc = convert_text("{\"k\":1}", Format::Fenced("json"), &Options::default()).unwrap();
    assert!(json_doc.markdown.contains("```json\n{\"k\":1}\n```"));
}

#[test]
fn epub_and_archive_callbacks_work() {
    let epub = minimal_epub();
    assert_eq!(Format::detect(&epub, None, None), Some(Format::Epub));
    assert!(matches!(
        convert(&epub, Format::Epub, &Options::default()),
        Err(Error::Unsupported(_))
    ));

    let html_cb = |xhtml: &str, _path: &str| {
        if xhtml.contains("Alpha") {
            "# Ch 1\n\nAlpha".to_string()
        } else {
            String::new()
        }
    };
    let epub_opts = Options {
        html: Some(&html_cb),
        ..Default::default()
    };
    let book = convert(&epub, Format::Epub, &epub_opts).unwrap();
    assert_eq!(book.title.as_deref(), Some("Book"));
    assert!(book.markdown.contains("# Ch 1\n\nAlpha"));

    let zip = build_zip(&[("sub/note.txt", b"inside note")]);
    assert_eq!(
        Format::detect(&zip, Some("bundle.zip"), None),
        Some(Format::Archive)
    );
    let listed = convert(&zip, Format::Archive, &Options::default()).unwrap();
    assert!(listed.markdown.contains("sub/note.txt"));

    let member_cb = |bytes: &[u8], name: &str, budget: usize| {
        let mut opts = Options::default();
        opts.limits.max_output_bytes = budget;
        if let Some(fmt) = Format::detect(bytes, Some(name), None) {
            convert(bytes, fmt, &opts).map(Some)
        } else {
            Ok(None)
        }
    };
    let archive_opts = Options {
        member: Some(&member_cb),
        ..Default::default()
    };
    let unpacked = convert(&zip, Format::Archive, &archive_opts).unwrap();
    assert!(unpacked.markdown.contains("inside note"));
}

#[test]
fn media_limits_deadline_and_error_recovery() {
    let docx_m = minimal_docx(true);
    let opts = Options::default();
    let items = media(&docx_m, Format::Docx, &opts).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].part, "word/media/logo.png");
    assert_eq!(items[0].format, "png");
    assert_eq!(items[0].bytes, 12);
    assert_eq!(items[0].hash.len(), 64);

    let raw = media_bytes(&docx_m, "word/media/logo.png", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(raw, b"\x89PNG\r\n\x1a\nfake");
    assert!(
        media_bytes(&docx_m, "missing.png", &opts)
            .unwrap()
            .is_none()
    );

    // Deadline timeout & clean error
    let expired = Options {
        deadline: Some(Instant::now() - Duration::from_secs(1)),
        ..Default::default()
    };
    let huge_csv = "a,b\n".repeat(5_000);
    assert_eq!(
        convert(huge_csv.as_bytes(), Format::Csv, &expired),
        Err(Error::Timeout)
    );
    assert_eq!(
        media_bytes(&docx_m, "word/media/logo.png", &expired),
        Err(Error::Timeout)
    );

    // Limits
    let tiny_limits = Options {
        limits: Limits {
            max_zip_entries: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        convert(&docx_m, Format::Docx, &tiny_limits),
        Err(Error::TooLarge("number of ZIP entries"))
    );

    // Error recovery + output budget + consecutive & concurrent safety
    assert!(convert(b"not a zip", Format::Docx, &opts).is_err());
    let plain_docx = minimal_docx(false);
    let ok = convert(&plain_docx, Format::Docx, &opts).unwrap();
    assert!(ok.markdown.contains("# Hello Title"));

    let truncated = convert_text(
        &"word ".repeat(500),
        Format::Text,
        &Options {
            limits: Limits {
                max_output_bytes: 64,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert!(truncated.markdown.len() <= 64);
    assert!(!truncated.warnings.is_empty());

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let d = plain_docx.clone();
            thread::spawn(move || {
                for j in 0..25 {
                    if (i + j) % 5 == 0 {
                        assert!(convert(b"bad", Format::Docx, &Options::default()).is_err());
                    }
                    let res = convert(&d, Format::Docx, &Options::default()).unwrap();
                    assert!(res.markdown.contains("# Hello Title"));
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
}
