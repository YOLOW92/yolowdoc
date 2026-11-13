<div align="center">

# yolowdoc

**Pure-Rust Documents-to-Markdown Library (`#![forbid(unsafe_code)]`)**

[**English**](README.md) · [**简体中文**](docs/README.zh-CN.md) · [**Changelog**](CHANGELOG.md)

[![Crates.io](https://img.shields.io/crates/v/yolowdoc.svg)](https://crates.io/crates/yolowdoc)
[![Docs.rs](https://docs.rs/yolowdoc/badge.svg)](https://docs.rs/yolowdoc)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust: 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](Cargo.toml)

</div>

A pure-Rust document, spreadsheet, presentation, e-book, email, archive, database, and source-code to Markdown library extracted from [PageSift](https://github.com/YOLOW92/pagesift) and released under MIT.

Built around self-written ZIP, XML (no external entity expansion), OLE/CFB, BIFF8, tar, LZMA/LZMA2, xz, bzip2, 7z, and SQLite B-tree readers (plus optional [`yolowzstd`](https://crates.io/crates/yolowzstd) for Zstandard archives)—zero C toolchains, `#![cfg_attr(not(test), forbid(unsafe_code))]`, content-first `Format::detect`, and explicit resource & timeout budgets (`Limits`, `Options::deadline`) for untrusted inputs.

---

## Highlights

- **100% Safe Rust & Zero C Dependencies**: Compiles on standard `std` targets with no `cc`, no `bindgen`, and no native C libraries (MSRV `1.89+`, `edition = "2024"`).
- **Wide Format Coverage**:
  - **Modern Office (OOXML)**: `.docx`/`.docm`, `.xlsx`/`.xlsm`, `.xlsb`, `.pptx`/`.pptm` (styles, tables, charts, SmartArt diagrams, OMML math, speaker notes, pivot tables, date/number formats).
  - **Legacy Binary Office & Hangul**: `.doc`, `.xls`, `.ppt` (OLE/CFB + BIFF8), `.hwp` (HWP 5), `.hwpx`.
  - **OpenDocument & WordPerfect**: `.odt`, `.ods`, `.odp`, Flat ODF (`.fodt`, `.fods`, `.fodp`, `.fodg`), `.wpd`/`.wp`/`.wp5`/`.wp6`.
  - **E-Books & Technical XML**: `.epub` (spine + Dublin Core metadata + DRM vs font-obfuscation check), `.mobi`/`.azw`/`.azw3`/`.prc`, `.fb2`, DocBook, JATS, OPML, SVG text, `.rtf`.
  - **Email & Archives**: `.eml` (RFC 5322 + MIME), `.mbox`, Outlook `.msg` (OLE + LZFu RTF), `.zip`, `.tar`, `.gz`/`.tgz`, `.bz2`/`.tbz2`, `.xz`/`.txz`, `.zst`/`.tzst` (optional `zstd` feature), `.7z`.
  - **Data, Notebooks & Code**: `.csv`/`.tsv`, `.dbf` (with `.dbt`/`.fpt` memo), `.sqlite`/`.db`/`.gpkg` (table dump without `libsqlite3`), `.ipynb`, `.srt`/`.vtt`, `.md`, plain text, and source code (optional structural splitting via [`yolowsyntax`](https://crates.io/crates/yolowsyntax)).
- **Untrusted-Input Budgets (`Limits`)**: Bounds ZIP entry count (`max_zip_entries`), per-entry & total decompressed bytes (`max_entry_bytes`, `max_total_bytes`), compression ratio (`max_ratio`), spreadsheet cells/columns (`max_cells`, `max_columns`), archive/attachment nesting depth, total output bytes (`max_output_bytes`), and cooperative deadlines (`Options::deadline`).
- **Composable Hooks**:
  - `Options::html`: delegates EPUB XHTML chapters and HTML email bodies to your own HTML-to-Markdown converter.
  - `Options::member`: delegates nested archive members and email attachments to your own recursive dispatcher (e.g. handling PDFs or nested documents under shared budgets).
- **Embedded Media Extraction**: `media()` and `media_bytes()` inspect and extract embedded images from `.docx`, `.pptx`, `.xlsx`, and `.epub` with SHA-256 content hashes and document locations (paragraph index, slide number, sheet/cell anchor, or EPUB chapter).

---

## Install

```toml
[dependencies]
yolowdoc = "0.1"
```

To select only a subset of optional features (for instance, omitting [`yolowsyntax`](https://crates.io/crates/yolowsyntax)'s embedded 41-language tables while keeping Zstandard support):

```toml
[dependencies]
yolowdoc = { version = "0.1", default-features = false, features = ["zstd"] }
```

---

## Quick Start

```rust
use yolowdoc::{Format, Options, convert};

fn main() -> Result<(), yolowdoc::Error> {
    let bytes = std::fs::read("report.docx").expect("read file");
    let format = Format::detect(&bytes, Some("report.docx"), None)
        .ok_or_else(|| yolowdoc::Error::Unsupported("unknown format".into()))?;
    let doc = convert(&bytes, format, &Options::default())?;
    println!("{}", doc.markdown);
    Ok(())
}
```

### Providing HTML and Recursive Member Converters

EPUB chapters, HTML email bodies, archive entries, and email attachments stay decoupled via callbacks on `Options`:

```rust
use yolowdoc::{Converted, Error, Format, Options, convert};

fn convert_member(bytes: &[u8], name: &str, max_bytes: usize) -> Result<Option<Converted>, Error> {
    let Some(fmt) = Format::detect(bytes, Some(name), None) else {
        return Ok(None);
    };
    let mut opts = Options {
        member: Some(&convert_member),
        ..Options::default()
    };
    opts.limits.max_output_bytes = max_bytes;
    convert(bytes, fmt, &opts).map(Some)
}
```

---

## Feature Flags

| Feature | Default | Description |
|---|---|---|
| `syntax` | Enabled | Enables [`yolowsyntax`](https://crates.io/crates/yolowsyntax) `0.1.2` for structural code splitting (functions/classes/sections) and symbol/import metadata on source files. Disable with `default-features = false` to omit embedded parse tables. |
| `zstd` | Enabled | Enables [`yolowzstd`](https://crates.io/crates/yolowzstd) `0.1.2` for `.zst`, `.zstd`, and `.tar.zst`/`.tzst` archive decompression under shared `Limits`. |

---

## Performance

Measured on Linux x86_64 (`cargo run --release --example bench`, pinned to a single core):

| Workload | Input | p50 Latency | Throughput |
|---|---|---|---|
| `docx_2500p` (DOCX 2,500 paras) | 41,454 B | 1.910 ms | 20.70 MiB/s |
| `xlsx_4000x8` (XLSX 32,000 cells) | 183,400 B | 15.212 ms | 11.50 MiB/s |
| `pptx_60s` (PPTX 60 slides) | 21,503 B | 0.507 ms | 40.41 MiB/s |
| `csv_10000r` (CSV 10,000 rows) | 580,399 B | 2.094 ms | 264.36 MiB/s |
| `csv_1M_rows` (CSV 1M rows, 16 MiB cap) | 64,037,065 B | 97.313 ms | 627.57 MiB/s |
| `sqlite_4800r` (SQLite B-tree) | 499,712 B | 1.782 ms | 267.47 MiB/s |
| `nested_archive_400f` (nested ZIP + TAR) | 228,346 B | 0.270 ms | 807.08 MiB/s |

Higher throughput / lower latency is better. Run `cargo run --release --example bench` locally to reproduce on your hardware.

---

## License

Licensed under the [MIT License](LICENSE).
