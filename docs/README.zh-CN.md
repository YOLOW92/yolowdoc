<div align="center">

# yolowdoc

**纯 Rust 文档转 Markdown 解析库（`#![forbid(unsafe_code)]`）**

[**English**](../README.md) · [**简体中文**](README.zh-CN.md) · [**更新日志**](../CHANGELOG.md)

[![Crates.io](https://img.shields.io/crates/v/yolowdoc.svg)](https://crates.io/crates/yolowdoc)
[![Docs.rs](https://docs.rs/yolowdoc/badge.svg)](https://docs.rs/yolowdoc)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../LICENSE)
[![Rust: 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](../Cargo.toml)

</div>

从 [PageSift](https://github.com/YOLOW92/pagesift) 独立拆出并以 **MIT 协议**开源的纯 Rust 文档解析与 Markdown 转换库。

内置自研的 ZIP、XML（不展开外部实体）、OLE/CFB、BIFF8、tar、LZMA/LZMA2、xz、bzip2、7z 与 SQLite B-tree 读取器（以及可选的 [`yolowzstd`](https://crates.io/crates/yolowzstd) 支援 Zstandard 压缩）——无需 C 工具链、全库 `#![cfg_attr(not(test), forbid(unsafe_code))]`，提供内容优先的 `Format::detect` 识别，以及面向不可信输入的资源上限与协作式超时控制（`Limits`、`Options::deadline`）。

---

## 特性一览

- **100% 安全 Rust，零 C 依赖**：无需 `cc`、`bindgen` 或本地 C 动态库，支持标准 `std` 目标（MSRV `1.89+`，`edition = "2024"`）。
- **广泛的格式支持**：
  - **现代 Office（OOXML）**：`.docx`/`.docm`、`.xlsx`/`.xlsm`、`.xlsb`、`.pptx`/`.pptm`（样式、表格、图表、SmartArt、OMML 公式、演讲者备注、透视表、日期与数字格式）。
  - **老式二进制 Office 与韩文文档**：`.doc`、`.xls`、`.ppt`（OLE/CFB + BIFF8）、`.hwp`（HWP 5）、`.hwpx`。
  - **OpenDocument 与 WordPerfect**：`.odt`、`.ods`、`.odp`、单文件 Flat ODF（`.fodt`、`.fods`、`.fodp`、`.fodg`）、`.wpd`/`.wp`/`.wp5`/`.wp6`。
  - **电子书与结构化 XML**：`.epub`（按 spine 顺序转换、读取 Dublin Core 元数据、区分 DRM 与字体混淆）、`.mobi`/`.azw`/`.azw3`/`.prc`、`.fb2`、DocBook、JATS、OPML、SVG 文字、`.rtf`。
  - **邮件与压缩包**：`.eml`（RFC 5322 + MIME）、`.mbox`、Outlook `.msg`（OLE + LZFu RTF）、`.zip`、`.tar`、`.gz`/`.tgz`、`.bz2`/`.tbz2`、`.xz`/`.txz`、`.zst`/`.tzst`（可选 `zstd` feature）、`.7z`。
  - **数据表、笔记本与代码**：`.csv`/`.tsv`、`.dbf`（支持 `.dbt`/`.fpt` 备注文件）、`.sqlite`/`.db`/`.gpkg`（纯 Rust 读取，不依赖 `libsqlite3`）、`.ipynb`、`.srt`/`.vtt`、`.md`、纯文本及代码文件（默认通过 [`yolowsyntax`](https://crates.io/crates/yolowsyntax) 按函数和类分段）。
- **不可信输入防护（`Limits`）**：限制 ZIP 条目数（`max_zip_entries`）、单条目与累计解压字节数（`max_entry_bytes`、`max_total_bytes`）、大条目压缩比（`max_ratio`）、表格单元格与列数（`max_cells`、`max_columns`）、压缩包/附件嵌套深度、输出总字节上限（`max_output_bytes`）与协作式截止时间（`Options::deadline`）。
- **可组合回调**：
  - `Options::html`：将 EPUB 的 XHTML 章节和邮件 HTML 正文交给调用方自己的 HTML 提取器。
  - `Options::member`：将压缩包成员与邮件附件交给调用方递归转换（例如衔接 PDF 解析或继续递归）。
- **内嵌媒体检查与提取**：`media()` 与 `media_bytes()` 可列出 `.docx`、`.pptx`、`.xlsx`、`.epub` 内的图片及其 SHA-256 哈希和引用位置（段落序号、幻灯片页码、工作表与锚点单元格、EPUB 章节）。

---

## 安装

```toml
[dependencies]
yolowdoc = "0.1"
```

如果希望按需开启可选组件（例如关闭 [`yolowsyntax`](https://crates.io/crates/yolowsyntax) 内嵌的 41 门语言语法表，仅保留 Zstandard 解压支持）：

```toml
[dependencies]
yolowdoc = { version = "0.1", default-features = false, features = ["zstd"] }
```

---

## 快速开始

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

---

## Feature 开关

| Feature | 默认 | 说明 |
|---|---|---|
| `syntax` | 开启 | 启用 [`yolowsyntax`](https://crates.io/crates/yolowsyntax) `0.1.2`，对源代码文件按顶层函数/类分段并写入符号和导入元数据；关闭后代码文件直接输出为带语言标记的代码块。 |
| `zstd` | 开启 | 启用 [`yolowzstd`](https://crates.io/crates/yolowzstd) `0.1.2`，在共享 `Limits` 配额下解压 `.zst`、`.zstd` 与 `.tar.zst`/`.tzst` 压缩文件。 |

---

## 协议

本项目采用 [MIT License](../LICENSE) 开源。
