# Changelog

## 0.1.1 - 2026-11-14

- Verify ISO-3309 CRC-32 checksums on ZIP entry reads and normalize leading-slash/case lookup keys consistently in `Zip::open`.
- Reduce XLSX worksheet parsing allocations and memory overhead (`CellVal` compact indexing into contiguous shared/inline string storage, zero-alloc `General` numeric fast path, buffer reuse in `xml::Reader::text_until_end_into`, and streamed `md::Grid` rendering).
- Add UTF-8 and UTF-16 LE/BE BOM decoding for plain text, Markdown, subtitles, notebooks, source code, and CSV/TSV inputs.
- Tighten bounds and state cleanup on untrusted inputs: cap repeating OLE DIFAT chain expansion, bound TNEF/LZFU preallocation, enforce `Limits` inside 7z `EncodedHeader`, check `Limits::max_columns` / `checked_add` in SQLite records, check `Options::deadline` in `media_bytes`, and clear DBF memo TLS via a `Drop` guard on panic.
- Add `.gitattributes` to enforce LF line endings across platforms and expand `examples/bench.rs`, integration tests, and ignored fuzz targets.

## 0.1.0 - 2026-11-13

- Initial standalone MIT-licensed release extracted from PageSift (`pagesift-office`).
- Pure-Rust (`#![cfg_attr(not(test), forbid(unsafe_code))]`, `edition = "2024"`, MSRV `1.89`) document-to-Markdown conversion with self-written ZIP, XML, OLE/CFB, BIFF8, tar, LZMA/LZMA2, xz, bzip2, 7z, and SQLite readers.
- Supports modern Office (`docx`, `xlsx`, `xlsb`, `pptx`), legacy Office (`doc`, `xls`, `ppt`), OpenDocument (`odt`, `ods`, `odp`, `fodt`/`fods`/`fodp`/`fodg`), Korean office (`hwp`, `hwpx`), e-books (`epub`, `mobi`/`azw`/`azw3`, `fb2`), XML/text formats (`csv`/`tsv`, `rtf`, `wpd`, `docbook`, `jats`, `opml`, `svg`, `ipynb`, `srt`/`vtt`, `txt`, `md`, source code), email (`eml`, `mbox`, `msg`), tabular/database (`dbf`, `sqlite`), and archives (`zip`, `tar`, `gz`, `bz2`, `xz`, `zst`/`tzst`, `7z`).
- Content-first `Format::detect` with fallback to filename and MIME type, bounded `Limits` (ZIP bomb, cell/column, nesting depth, and `max_output_bytes`), cooperative `deadline`, embedded `media` / `media_bytes` inspection, and callback hooks (`Options::html`, `Options::member`).
- Optional default features: `syntax` ([`yolowsyntax`](https://crates.io/crates/yolowsyntax) `0.1.2` for structural code-outline splitting and symbol/import metadata extraction) and `zstd` ([`yolowzstd`](https://crates.io/crates/yolowzstd) `0.1.2` for Zstandard single-file and `.tar.zst` archive decompression).
