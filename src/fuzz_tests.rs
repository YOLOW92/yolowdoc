//! 内部解析器的变异 Fuzz 测试（绕过公开入口的 `catch_unwind`，直接暴露 panic、死循环与异常分配）。
//! 通过 `cargo test --release fuzz_mutating -- --ignored --nocapture` 运行。

use std::time::{Duration, Instant};

use crate::{Format, Limits, Options, test_probe};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next() as usize) % bound
        }
    }
}

fn mutate(seed: &[u8], rng: &mut Rng, max_len: usize) -> Vec<u8> {
    let mut out = seed.to_vec();
    let ops = 1 + rng.usize(4);
    for _ in 0..ops {
        let kind = rng.usize(6);
        if kind == 0 && !out.is_empty() {
            let i = rng.usize(out.len());
            out[i] ^= (rng.next() & 0xFF) as u8;
        } else if kind == 1 && !out.is_empty() {
            const INTERESTING: [u32; 8] = [
                0,
                1,
                0x7F,
                0xFF,
                0xFFFF,
                0x8000_0000,
                0x7FFF_FFFF,
                0xFFFF_FFFF,
            ];
            let i = rng.usize(out.len());
            let v = INTERESTING[rng.usize(INTERESTING.len())].to_le_bytes();
            let take = v.len().min(out.len() - i);
            out[i..i + take].copy_from_slice(&v[..take]);
        } else if kind == 2 && out.len() > 2 {
            let keep = rng.usize(out.len());
            out.truncate(keep);
        } else if kind == 3 && out.len() < max_len {
            let i = rng.usize(out.len() + 1);
            let val = (rng.next() & 0xFF) as u8;
            let n = 1 + rng.usize(16);
            for _ in 0..n {
                if out.len() >= max_len {
                    break;
                }
                out.insert(i, val);
            }
        } else if kind == 4 && out.len() > 4 {
            let a = rng.usize(out.len());
            let b = (a + 1 + rng.usize(16)).min(out.len());
            out.drain(a..b);
        } else if !out.is_empty() && out.len() < max_len {
            let a = rng.usize(out.len());
            let b = (a + 1 + rng.usize(32)).min(out.len());
            let chunk = out[a..b].to_vec();
            let dst = rng.usize(out.len() + 1);
            if out.len() + chunk.len() <= max_len {
                out.splice(dst..dst, chunk);
            }
        }
    }
    out
}

fn ole_single(name: &str, data: &[u8]) -> Vec<u8> {
    crate::ole::tests::build(&[crate::ole::tests::Node {
        name,
        data,
        storage: false,
        parent: 0,
    }])
}

fn ole_pair(a: (&str, &[u8]), b: (&str, &[u8])) -> Vec<u8> {
    crate::ole::tests::build(&[
        crate::ole::tests::Node {
            name: a.0,
            data: a.1,
            storage: false,
            parent: 0,
        },
        crate::ole::tests::Node {
            name: b.0,
            data: b.1,
            storage: false,
            parent: 0,
        },
    ])
}

fn fuzz_target(name: &str, seeds: &[Vec<u8>], iters: usize, rng: &mut Rng, f: impl Fn(&[u8])) {
    let mut max_peak = 0usize;
    let mut max_time = Duration::ZERO;
    let mut corpus = seeds.to_vec();
    for i in 0..iters {
        let base = &corpus[rng.usize(corpus.len())];
        let input = if i < seeds.len() {
            seeds[i].clone()
        } else {
            mutate(base, rng, 256 * 1024)
        };
        let _guard = crate::budget::enter(4 << 20);
        let t0 = Instant::now();
        let (_, peak) = test_probe::peak(|| f(&input));
        let dt = t0.elapsed();
        if peak > max_peak {
            max_peak = peak;
        }
        if dt > max_time {
            max_time = dt;
        }
        assert!(
            dt < Duration::from_secs(2),
            "fuzz target `{name}` iter {i} took {:?} (>2s) on {} bytes (head {:02x?})",
            dt,
            input.len(),
            &input[..input.len().min(32)]
        );
        assert!(
            peak <= 256 * 1024 * 1024,
            "fuzz target `{name}` iter {i} peaked at {} bytes (>256 MiB) on {} bytes (head {:02x?})",
            peak,
            input.len(),
            &input[..input.len().min(32)]
        );
        if i % 50 == 0 && corpus.len() < 64 {
            corpus.push(input);
        }
    }
    eprintln!(
        "fuzz {name:<14} {iters:>5} iters ok | max_peak={:>9} B | max_time={:>8.3} ms",
        max_peak,
        max_time.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "mutating fuzz campaign; run with --ignored --nocapture"]
fn fuzz_mutating_internal_parsers() {
    let mut rng = Rng(0x5EED_F022_1234_5678);
    let opts = Options::default();
    let limits = Limits::default();

    // 1. ZIP
    let zip1 = crate::zip::tests::build(&[
        ("a.txt", b"hello", false),
        (
            "word/document.xml",
            b"<w:document><w:body><w:p><w:r><w:t>hi</w:t></w:r></w:p></w:body></w:document>",
            true,
        ),
    ]);
    fuzz_target("zip", std::slice::from_ref(&zip1), 3000, &mut rng, |b| {
        if let Ok(z) = crate::zip::Zip::open(b, &limits) {
            let names: Vec<String> = z.names().take(16).map(|(n, _)| n.to_owned()).collect();
            for n in names {
                let _ = z.read(&n);
            }
        }
    });

    // 2. XML
    let xml1 =
        br#"<?xml version="1.0"?><root a="1" b='x&amp;y'><child>hi&#65;&lt;z&gt;</child></root>"#
            .to_vec();
    let xml2 = br#"<t>&apos;&quot;&x;&#x1f600;</t>"#.to_vec();
    fuzz_target("xml", &[xml1, xml2], 4000, &mut rng, |b| {
        let s = crate::xml::decode(b.to_vec());
        let mut r = crate::xml::Reader::new(&s);
        while let Ok(Some(_)) = r.next() {}
    });

    // 3. OLE/CFB
    let ole1 = ole_pair(("WordDocument", b"hello"), ("1Table", b"world"));
    fuzz_target("ole", &[ole1], 3000, &mut rng, |b| {
        if let Ok(o) = crate::ole::Ole::open(b) {
            for e in o.children(None).into_iter().take(16) {
                if e.is_stream {
                    let _ = o.read(e);
                }
            }
        }
    });

    // 4. XLSX
    let xlsx1 = crate::zip::tests::build(&[
        ("[Content_Types].xml", b"<Types/>", false),
        ("xl/workbook.xml", b"<workbook><sheets><sheet name=\"S1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>", false),
        ("xl/_rels/workbook.xml.rels", b"<Relationships><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>", false),
        ("xl/sharedStrings.xml", b"<sst><si><t>A</t></si></sst>", false),
        ("xl/worksheets/sheet1.xml", b"<worksheet><sheetData><row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c></row></sheetData></worksheet>", false),
    ]);
    fuzz_target("xlsx", &[xlsx1], 2000, &mut rng, |b| {
        if let Ok(z) = crate::zip::Zip::open(b, &limits) {
            let _ = crate::xlsx::convert(&z, &opts);
        }
    });

    // 5. SQLite（复用真实多表/WITHOUT ROWID/溢出页样本）
    let sq1 = crate::sqlite::tests::sample();
    fuzz_target("sqlite", &[sq1], 4000, &mut rng, |b| {
        let _ = crate::sqlite::convert(b, &opts);
    });

    // 6. DOCX / PPTX / XLSB / ODF
    let docx1 = crate::zip::tests::build(&[(
        "word/document.xml",
        br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#,
        false,
    )]);
    fuzz_target("docx", &[docx1], 2000, &mut rng, |b| {
        if let Ok(z) = crate::zip::Zip::open(b, &limits) {
            let _ = crate::docx::convert(&z, &opts);
        }
    });
    let pptx1 = crate::zip::tests::build(&[
        ("ppt/presentation.xml", br#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="r1"/></p:sldIdLst></p:presentation>"#, false),
        ("ppt/_rels/presentation.xml.rels", br#"<Relationships><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/></Relationships>"#, false),
        ("ppt/slides/slide1.xml", br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>S1</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#, false),
    ]);
    fuzz_target("pptx", &[pptx1], 2000, &mut rng, |b| {
        if let Ok(z) = crate::zip::Zip::open(b, &limits) {
            let _ = crate::pptx::convert(&z, &opts);
        }
    });
    let xlsb_wb: Vec<u8> = [131, 0, 1, 0, 156, 0, 0, 0, 0, 0].into_iter().collect();
    let xlsb1 = crate::zip::tests::build(&[
        ("xl/workbook.bin", &xlsb_wb, false),
        ("xl/worksheets/sheet1.bin", &[0x91, 0, 0], false),
    ]);
    fuzz_target("xlsb", &[xlsb1], 2000, &mut rng, |b| {
        if let Ok(z) = crate::zip::Zip::open(b, &limits) {
            let _ = crate::legacy::xlsb::convert(&z, &opts);
        }
    });
    let odt1 = crate::zip::tests::build(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text", false),
        ("content.xml", br#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:body><office:text><text:h text:outline-level="1">T</text:h><text:p>Hi</text:p></office:text></office:body></office:document-content>"#, false),
    ]);
    fuzz_target("odf", &[odt1], 2000, &mut rng, |b| {
        if let Ok(z) = crate::zip::Zip::open(b, &limits) {
            let _ = crate::odf::convert(&z, Format::Odt, &opts);
        }
    });

    // 6. Legacy Doc/Xls/Ppt/Hwp
    fuzz_target(
        "legacy_doc",
        &[ole_single("WordDocument", &[0u8; 512])],
        1500,
        &mut rng,
        |b| {
            let _ = crate::legacy::doc::convert(b, &opts);
        },
    );
    fuzz_target(
        "legacy_xls",
        &[ole_single("Workbook", &[0x09, 0x08, 2, 0, 0, 0])],
        1500,
        &mut rng,
        |b| {
            let _ = crate::legacy::xls::convert(b, &opts);
        },
    );
    fuzz_target(
        "legacy_ppt",
        &[ole_pair(
            ("PowerPoint Document", &[0; 32]),
            ("Current User", &[0; 32]),
        )],
        1500,
        &mut rng,
        |b| {
            let _ = crate::legacy::ppt::convert(b, &opts);
        },
    );
    fuzz_target(
        "legacy_hwp",
        &[ole_single(
            "FileHeader",
            b"HWP Document File\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        )],
        1500,
        &mut rng,
        |b| {
            let _ = crate::legacy::hwp::convert(b, &opts);
        },
    );

    // 7. Compress / Tar / Archive
    let tar1 = crate::tar::tests::file("a.txt", b"hello", b'0');
    fuzz_target(
        "tar_gz",
        &[tar1.clone(), vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 3, 0]],
        3000,
        &mut rng,
        |b| {
            let _ = crate::tar::entries(b, 64);
            let _ = crate::tar::gunzip(b, 4 << 20);
        },
    );
    let bz = include_bytes!("../tests/data/compress/text.bz2").to_vec();
    let xz = include_bytes!("../tests/data/compress/text.xz").to_vec();
    let sz = include_bytes!("../tests/data/compress/files.7z").to_vec();
    fuzz_target("bzip2", &[bz], 2000, &mut rng, |b| {
        let _ = crate::compress::bzip2::decode(b, 4 << 20);
    });
    fuzz_target("xz", &[xz], 2000, &mut rng, |b| {
        let _ = crate::compress::xz::decode(b, 4 << 20);
    });
    fuzz_target("sevenz", &[sz], 2000, &mut rng, |b| {
        let _ = crate::compress::sevenz::decode(b, 4 << 20);
    });

    // 8. RTF / WordPerfect / Email / Archive / Detect
    fuzz_target(
        "rtf",
        &[br"{\rtf1\ansi {\b hi}\par world}".to_vec()],
        2000,
        &mut rng,
        |b| {
            let _ = crate::rtf::convert(b, &opts);
        },
    );
    fuzz_target(
        "wordperfect",
        &[b"\xFFWPC\x0e\0\0\0\x01\x0a\0\0\0\0Hello\x0dworld".to_vec()],
        2000,
        &mut rng,
        |b| {
            let _ = crate::wordperfect::convert(b, &opts);
        },
    );
    fuzz_target(
        "lzfu",
        &[vec![
            0x2d, 0, 0, 0, 0x2b, 0, 0, 0, 0x4c, 0x5a, 0x46, 0x75, 0, 0, 0, 0,
        ]],
        2000,
        &mut rng,
        |b| {
            let _ = crate::email::lzfu::decompress(b);
        },
    );
    let eml1 = b"From: a@b.c\r\nSubject: Hi\r\nContent-Type: text/plain\r\n\r\nBody\r\n".to_vec();
    fuzz_target("email_mime", &[eml1], 2000, &mut rng, |b| {
        let _ = crate::email::convert_eml(b, &opts);
    });
    fuzz_target(
        "email_msg",
        &[ole_single("__substg1.0_0037001F", b"S\0u\0b\0")],
        1500,
        &mut rng,
        |b| {
            let _ = crate::email::convert_msg(b, &opts);
        },
    );
    fuzz_target("archive", &[tar1, zip1.clone()], 2000, &mut rng, |b| {
        let _ = crate::archive::convert(b, &opts);
    });
    fuzz_target("detect", &[zip1], 2000, &mut rng, |b| {
        let _ = Format::detect(b, None, None);
    });
}
