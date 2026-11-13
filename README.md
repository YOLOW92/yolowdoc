<div align="center">

# yolowdoc

**Pure-Rust document parsers with unsafe-free production code and no native C dependencies.**

[**English**](README.md) · [**简体中文**](docs/README.zh-CN.md) · [**Changelog**](CHANGELOG.md)

[![Crates.io](https://img.shields.io/crates/v/yolowdoc.svg)](https://crates.io/crates/yolowdoc)
[![Docs.rs](https://docs.rs/yolowdoc/badge.svg)](https://docs.rs/yolowdoc)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust: 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](Cargo.toml)

</div>

`yolowdoc` is a standalone MIT-licensed document-to-Markdown library in pure Rust (originally extracted from PageSift). It converts Office files, OpenDocument suites, e-books, emails, databases, archives, structured text, and source code into Markdown with content-first format detection (`Format::detect`) and configurable resource budgets (`Limits`, `Options::deadline`).

Its production crate code enforces `#![cfg_attr(not(test), forbid(unsafe_code))]`, requires no C toolchain (`cc`, `bindgen`, or native shared libraries), and targets Rust `1.89+` (`edition = "2024"`). Core container and format readers (ZIP central directory, XML without external entity expansion, OLE/CFB, BIFF8, tar, LZMA/LZMA2, xz, bzip2, 7z, and SQLite B-tree) are implemented directly in this crate, paired with pure-Rust dependencies: `miniz_oxide` (Deflate), `memchr`, `encoding_rs`, `sha2`, `serde_json`, and optional `yolowsyntax` / `yolowzstd`.

---

## Highlights

- **Pure Rust & No Native C Dependencies**: Compiles with standard Cargo toolchains (`edition = "2024"`, MSRV `1.89`). Production code in `yolowdoc` forbids `unsafe` (`#![cfg_attr(not(test), forbid(unsafe_code))]`).
- **XLSX Memory & Allocation Optimization (`0.1.1`)**: Uses a compact `CellVal` enum indexing shared strings by ID (avoiding repeated cloning of shared strings), a contiguous string store, reused XML text buffers (`text_until_end_into`), a fast path for `"General"` numeric formatting, compact `Grid` table rendering, and batched output-budget checks.
- **ZIP Integrity & Defensive Parsing**: Verifies ISO-3309 CRC-32 checksums on `Zip::read()` with normalized lookup keys, rejects external XML entity expansion, bounds malformed repeating OLE DIFAT chains and TNEF/LZFu preallocations, and checks SQLite record arithmetic for overflow.
- **Deterministic BOM & Caller-Decoded Text**: Decodes UTF-8 BOM and UTF-16 LE/BE BOM streams automatically for text-family documents and CSV/TSV; callers with external charset knowledge can pass pre-decoded UTF-8 directly to `convert_text()` and `convert_csv_text()`.
- **Composable Hooks & Media Extraction**: Keeps HTML-to-Markdown conversion (`Options::html`), nested archive/attachment recursion (`Options::member`), and embedded media inspection (`media()`, `media_bytes()`) cleanly decoupled.

---

## Supported Formats & Actual Capabilities

| Category | Formats | Extracted Content & Scope |
|---|---|---|
| **Word Processing (Modern & Legacy)** | `.docx`/`.docm`, `.doc`, `.odt`, `.fodt`, `.rtf`, `.wpd`/`.wp`/`.wp5`/`.wp6`, `.hwp`, `.hwpx` | **DOCX**: style-based headings, `numbering.xml` lists, tables with merged-cell unwrapping, links, image alt-text/part paths, footnotes/endnotes, textboxes, OMML math to LaTeX (headers/footers, comments, and deleted revisions omitted). **DOC** (MS-DOC / Word 6–95): CLX pieces, headings, lists, field results, tables, footnotes, textboxes. **ODT/FODT**, **HWP/HWPX**, **RTF**, **WordPerfect**: headings, paragraphs, lists, tables, metadata. |
| **Spreadsheets** | `.xlsx`/`.xlsm`, `.xlsb`, `.xls`, `.ods`, `.fods` | **XLSX**: shared/inline strings, dates, custom number & pivot display formatting, hidden row/col skipping, bounding rectangle on sparse sheets, cached formula values (no formula recalculation; charts/images/comments omitted from Markdown text—use `media()`/`media_bytes()` for embedded images). **XLSB** (BIFF12) & **XLS** (BIFF8): sheets, shared strings, number/date formats, cached formula values. **ODS/FODS**: sheets and bounded repeated rows/cells. |
| **Presentations** | `.pptx`/`.pptm`, `.ppt`, `.odp`, `.fodp` | **PPTX**: slide order, titles, placeholder/master bullet inheritance, tables, image alt-text, SmartArt diagrams, cached chart data, speaker notes. **PPT** (MS-PPT): persistent directory / sequential fallback text, lists, notes. **ODP/FODP**: slide text, lists, tables. |
| **E-Books & XML Docs** | `.epub`, `.mobi`/`.azw`/`.azw3`/`.prc`, `.fb2`, DocBook, JATS, OPML, SVG | **EPUB**: spine order, Dublin Core metadata, font-obfuscation vs DRM check; **requires `Options::html` callback** (returns `Error::Unsupported` if unset). **MOBI/AZW/PRC**, **FB2**, **DocBook**, **JATS**, **OPML**, **SVG**: structured sections, headings, lists, tables, metadata/text. |
| **Email** | `.eml`, `.mbox`, `.msg` | RFC 5322/MIME headers & bodies, `mbox` message splitting, Outlook OLE + LZFu RTF `.msg`. HTML bodies use `Options::html` when set and fall back to plain text when unset; attachments use `Options::member` (listing only if unset). |
| **Tabular & Databases** | `.csv`, `.tsv`, `.dbf`, `.sqlite`/`.db`/`.gpkg` | Delimiter-sniffed `.csv`/`.tsv`, `.sqlite`/`.db`/`.gpkg` schema & B-tree table dump (including `WITHOUT ROWID` and overflow pages without `libsqlite3`). `.dbf` with companion `.dbt`/`.fpt` memo extraction **inside archives** (`convert()` on standalone `.dbf` bytes does not read sidecar disk files). |
| **Archives** | `.zip`, `.tar`, `.gz`/`.tgz`, `.bz2`/`.tbz2`, `.xz`/`.txz`, `.zst`/`.tzst`, `.7z` | Single-file decompression and container listing; optional recursive content conversion via `Options::member` (member listing only when `Options::member` is unset). `.zst`/`.tar.zst` requires the `zstd` feature. |
| **Notebooks, Text & Code** | `.ipynb`, `.srt`/`.vtt`, `.md`, `.txt`, source files | Jupyter notebook cells & outputs, SubRip/WebVTT deduplicated captions, front-matter Markdown/text (UTF-8/UTF-16 BOM), and fenced source code (structural function/class splitting with `syntax` feature). |

---

## Install

```toml
[dependencies]
yolowdoc = "0.1"
```

To select a smaller feature set (for example, omitting [`yolowsyntax`](https://crates.io/crates/yolowsyntax)'s language grammar tables while keeping Zstandard decompression):

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

For input bytes already decoded into UTF-8 by an external detector or HTTP header, use `convert_text(&text, format, &opts)` or `convert_csv_text(&text, &opts)`.

---

## Callbacks & Embedded Media

- **`Options::html` (`Fn(&str, &str) -> String + Sync`)**: Required for EPUB (unprovided EPUB conversions return `Error::Unsupported`). Also used for HTML email bodies when present, falling back to the plain-text part if omitted.
- **`Options::member` (`Fn(&[u8], &str, usize) -> Result<Option<Converted>, Error> + Sync`)**: Used for archive members and email attachments, receiving the remaining output byte budget. When omitted, archives list member paths/sizes and emails list attachment metadata.
- **`media()` & `media_bytes()`**: Inspect and extract embedded image parts from `.docx`, `.pptx`, `.xlsx`, and `.epub` with SHA-256 hashes and document locations.

```rust
use yolowdoc::{Converted, Error, Format, Options, convert, media, media_bytes};

// Wire in your HTML-to-Markdown converter (such as an HTML DOM/reader crate).
fn render_html_to_markdown(xhtml: &str, _archive_path: &str) -> String {
    todo!("pass `{}` through your HTML-to-Markdown converter", xhtml.len())
}

fn convert_member(
    bytes: &[u8],
    name: &str,
    remaining_budget: usize,
) -> Result<Option<Converted>, Error> {
    let Some(fmt) = Format::detect(bytes, Some(name), None) else {
        return Ok(None);
    };
    let mut opts = Options {
        html: Some(&render_html_to_markdown),
        member: Some(&convert_member),
        ..Options::default()
    };
    opts.limits.max_output_bytes = remaining_budget;
    convert(bytes, fmt, &opts).map(Some)
}

fn run_with_hooks(bytes: &[u8], filename: &str) -> Result<(), Error> {
    let fmt = Format::detect(bytes, Some(filename), None)
        .ok_or_else(|| Error::Unsupported("unknown format".into()))?;
    let opts = Options {
        html: Some(&render_html_to_markdown),
        member: Some(&convert_member),
        ..Options::default()
    };
    let doc = convert(bytes, fmt, &opts)?;
    println!("{}", doc.markdown);

    for item in media(bytes, fmt, &opts)? {
        if let Some(raw) = media_bytes(bytes, &item.part, &opts)? {
            println!("{} ({}, {} bytes, sha256={})", item.part, item.format, raw.len(), item.hash);
        }
    }
    Ok(())
}
```

---

## Feature Flags

| Feature | Default | Description |
|---|---|---|
| `syntax` | Enabled | Enables [`yolowsyntax`](https://crates.io/crates/yolowsyntax) `0.1.2` for structural code splitting (functions/classes/sections) and symbol/import metadata on source files. Disable with `default-features = false` to emit plain fenced blocks without embedded grammar tables. |
| `zstd` | Enabled | Enables [`yolowzstd`](https://crates.io/crates/yolowzstd) `0.1.2` for `.zst`, `.zstd`, and `.tar.zst`/`.tzst` decompression under shared `Limits`. |

---

## Benchmarks

### Synthetic Document Benchmark (`examples/bench.rs`)

- **Version tested**: `yolowdoc 0.1.1`
- **Environment**: `Intel(R) Core(TM) i7-10700 CPU @ 2.90GHz` (x86_64), `Linux 7.0.0-43-generic`, `rustc 1.99.0`
- **Build & Pinning**: `--release` profile (`lto = "fat"`, `codegen-units = 1`), single-core pinned (`taskset -c 4`), 25 timed rounds after warmup (5 rounds for heavy workloads)
- **Throughput definition**: `input_bytes / 1024^2 / (p50_ms / 1000)` based on total input byte size. `VmHWM` reflects cumulative process peak RSS across the whole benchmark binary, not isolated per-document RSS.

| Workload | Description | Input | Output | p50 Latency | p95 Latency | Throughput |
|---|---|---|---|---|---|---|
| `docx_200p` | DOCX, 200 paragraphs | 34,494 B | 25,945 B | 0.170 ms | 0.185 ms | 193.53 MiB/s |
| `docx_2500p` | DOCX, 2,500 paragraphs | 41,454 B | 326,753 B | 1.910 ms | 1.950 ms | 20.70 MiB/s |
| `docx_20000p` | DOCX, 20,000 paragraphs | 95,587 B | 2,631,653 B | 15.825 ms | 16.275 ms | 5.76 MiB/s |
| `xlsx_200x6` | XLSX, 200 × 6 cells | 8,015 B | 11,017 B | 0.603 ms | 0.632 ms | 12.68 MiB/s |
| `xlsx_4000x8` | XLSX, 4,000 × 8 cells | 183,400 B | 308,448 B | 15.212 ms | 15.259 ms | 11.50 MiB/s |
| `xlsx_5k_rep_512B` | XLSX, 5,000 refs to 512 B shared string | 254,260 B | 2,585,015 B | 4.240 ms | 4.416 ms | 57.19 MiB/s |
| `xlsx_25000x4` | XLSX, 25,000 × 4 cells | 603,687 B | 1,176,901 B | 46.726 ms | 60.611 ms | 12.32 MiB/s |
| `pptx_60s` | PPTX, 60 slides | 21,503 B | 5,123 B | 0.507 ms | 0.594 ms | 40.41 MiB/s |
| `pptx_300s` | PPTX, 300 slides | 106,209 B | 26,567 B | 2.465 ms | 2.481 ms | 41.09 MiB/s |
| `media_40img` | DOCX media scan, 40 × 16 KiB PNGs | 668,676 B | 40 items | 2.720 ms | 2.752 ms | 234.43 MiB/s |
| `csv_10000r` | CSV, 10,000 rows × 5 columns | 580,399 B | 680,443 B | 2.094 ms | 2.141 ms | 264.36 MiB/s |
| `csv_1M_rows` | CSV, 1M input rows, budget-limited conversion (`max_output_bytes = 16 MiB`, `max_cells = 2,000,000`, 5 cols/row) | 64,037,065 B | 16,777,204 B (capped) | 97.313 ms | 98.006 ms | 627.57 MiB/s* |
| `sqlite_4800r` | SQLite, 4,800 rows across interior + leaf B-tree pages | 499,712 B | 283,258 B | 1.782 ms | 1.839 ms | 267.47 MiB/s |
| `zip_200f` | ZIP, 200 text entries | 22,312 B | 6,645 B | 0.075 ms | 0.078 ms | 284.05 MiB/s |
| `tar_200f` | TAR, 200 text entries | 205,824 B | 6,645 B | 0.064 ms | 0.069 ms | 3,059.23 MiB/s |
| `nested_archive_400f` | Nested ZIP + TAR member listing (400 entries; inner callback omits `member` propagation) | 228,346 B | 13,438 B | 0.270 ms | 0.289 ms | 807.08 MiB/s |

\* `csv_1M_rows` hits the default `max_output_bytes` (`16 MiB`) budget before converting all 1,000,000 rows; reported throughput divides full input size by `p50` latency and does not represent full 1M-row Markdown serialization throughput.

Lower latency and higher throughput are better. In hotspot attribution (`perf_micro_attribution`, `0.1.1` vs `0.1.0`), `xlsx_4000x8` allocations dropped from 145,131 to 1,137 while latency dropped from 18.856 ms to 15.212 ms.

---

## Testing & Validation

The repository includes multi-layered verification:

- **Unit & Integration Suites**: Module-level unit tests across containers and parsers (171 unit tests with default features) plus public-API integration tests ([`tests/integration.rs`](tests/integration.rs)) and doctests.
- **External Real-Document Manifest**: [`tests/corpora.tsv`](tests/corpora.tsv) catalogs **72** SHA-256 verified public documents (`20` DOCX, `16` XLSX, `16` PPTX, `16` CSV, `4` EPUB) with source URLs, licenses, byte sizes, and recorded output sizes (note: `out_bytes` and `warnings` in the manifest are execution snapshots, not human-annotated structural accuracy ground truth).
- **Mutating Fuzz Campaign**: [`src/fuzz_tests.rs`](src/fuzz_tests.rs) (`#[ignore]`) exercises 24 internal parser targets directly (bypassing top-level unwind boundaries) against length/offset/boundary mutations.
- **Microbenchmarks & Full-Doc Benchmarks**: [`src/bench_internal.rs`](src/bench_internal.rs) (`#[ignore]`) measures allocation counts, peak live heap bytes, and latency; [`examples/bench.rs`](examples/bench.rs) runs end-to-end workloads.
- **CI Matrix & Feature Combinations**: [`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs formatting, Clippy, debug tests, `--no-default-features`, `--no-default-features --features syntax`, `--no-default-features --features zstd`, and default features across `ubuntu-latest` (Rust `1.89.0` & `stable`), `macos-latest` (`stable`), and `windows-latest` (`stable`).

```bash
# Standard test suite & all feature combinations
cargo test --release --locked
cargo test --release --locked --no-default-features
cargo test --release --locked --no-default-features --features syntax
cargo test --release --locked --no-default-features --features zstd

# Benchmarks & mutating fuzz campaign
cargo run --release --example bench
cargo test --release fuzz_mutating -- --ignored --nocapture
cargo test --release perf_micro_attribution -- --ignored --nocapture
```

---

## Resource Limits & Boundaries

Configurable via `Limits` on `Options::limits`:

| Field | Default | Scope & Notes |
|---|---|---|
| `max_zip_entries` | `20_000` | Maximum central-directory entries accepted in a ZIP container. |
| `max_entry_bytes` | `256 MiB` | Maximum uncompressed size of a single ZIP/archive/document part. |
| `max_total_bytes` | `512 MiB` | Maximum cumulative decompressed bytes read across parts of one document. |
| `max_ratio` | `400` | Compression-ratio limit applied to ZIP entries larger than `4 MiB` (ZIP-specific guard, not a uniform ratio limit across all compression formats). |
| `max_cells` | `2_000_000` | Maximum rendered table cells for spreadsheets and CSV; excess rows are truncated with a warning. |
| `max_columns` | `1_024` | Maximum spreadsheet/SQLite columns per table (OOXML spec max is 16,384); excess columns are dropped with a warning. |
| `max_output_bytes` | `16 MiB` | Output text budget for returned Markdown; partial output is returned with an `output_truncated` warning. |

Additional operational limits and guarantees:

- **Fixed Nesting & Member Caps**: Recursive archive/email attachment nesting is fixed internally at **3 levels**, and shared processed member/message count across archives and mbox streams is fixed at **1,000**.
- **Cooperative Deadline (`Options::deadline`)**: Checked at structural boundaries (paragraphs, rows, slides, chapters, members, media reads) and returns `Error::Timeout` when exceeded; it is cooperative, not preemptive cancellation.
- **Output Budget vs Process Memory**: `max_output_bytes` bounds emitted Markdown text size and helps curtail conversion work early, but it is **not** a hard ceiling on total process RSS or virtual memory for arbitrary untrusted inputs.

---

## License

Licensed under the [MIT License](LICENSE).
