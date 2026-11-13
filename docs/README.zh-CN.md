<div align="center">

# yolowdoc

**纯 Rust 文档解析库：生产代码禁用 `unsafe`，零原生 C 依赖。**

[**English**](../README.md) · [**简体中文**](README.zh-CN.md) · [**更新日志**](../CHANGELOG.md)

[![Crates.io](https://img.shields.io/crates/v/yolowdoc.svg)](https://crates.io/crates/yolowdoc)
[![Docs.rs](https://docs.rs/yolowdoc/badge.svg)](https://docs.rs/yolowdoc)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../LICENSE)
[![Rust: 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](../Cargo.toml)

</div>

`yolowdoc` 是一个从 PageSift 独立拆出并以 **MIT 协议**开源的纯 Rust 文档转 Markdown 库，覆盖办公文档、OpenDocument 套件、电子书、邮件、数据库、压缩包、结构化文本与源代码，并提供基于文件内容的 `Format::detect` 识别以及可配置的资源预算和截止时间控制（`Limits`、`Options::deadline`）。

项目生产代码启用 `#![cfg_attr(not(test), forbid(unsafe_code))]`，无需 C 工具链（`cc`、`bindgen` 或本地 C 动态库），支持 Rust `1.89+`（`edition = "2024"`）。核心容器与格式读取器（ZIP 中央目录、禁止外部实体展开的 XML、OLE/CFB、BIFF8、tar、LZMA/LZMA2、xz、bzip2、7z、SQLite B-tree）由本项目直接实现，配合纯 Rust 第三方依赖：`miniz_oxide`（Deflate）、`memchr`、`encoding_rs`、`sha2`、`serde_json`，以及可选的 `yolowsyntax` 和 `yolowzstd`。

---

## 核心特点

- **纯 Rust、零原生 C 依赖**：可在标准 Cargo 环境下直接编译（`edition = "2024"`，MSRV `1.89`）；`yolowdoc` 自身生产代码严格执行 `#![cfg_attr(not(test), forbid(unsafe_code))]`。
- **XLSX 内存与分配优化（`0.1.1`）**：采用紧凑 `CellVal` 枚举按索引引用 shared strings（避免重复克隆共享字符串）、连续文本存储、XML 读取缓冲区复用（`text_until_end_into`）、`"General"` 数值格式快路径、紧凑 `Grid` 表格存储与渲染，并按批检查输出预算以降低中间分配开销。
- **ZIP 完整性与边界防护**：在 `Zip::read()` 阶段执行 ISO-3309 CRC-32 校验并统一归一化前导斜杠与大小写索引；限制畸形循环 OLE DIFAT 链、TNEF/LZFu 预分配大小，并对 SQLite 记录长度做溢出检查。
- **确定性 BOM 与预解码文本入口**：对文本类格式及 CSV/TSV 自动解码 UTF-8 BOM 与 UTF-16 LE/BE BOM；若调用方已知字符集并提前完成转码，可直接调用 `convert_text()` 和 `convert_csv_text()` 传入 UTF-8 字符串。
- **可组合回调与媒体提取**：通过 `Options::html` 与 `Options::member` 解耦 HTML 转 Markdown 和嵌套归档/附件递归转换，并通过 `media()` / `media_bytes()` 独立检查与提取内嵌图片。

---

## 格式支持与实际能力

| 类别 | 支持格式 | 实际提取能力与范围 |
|---|---|---|
| **文字处理（现代与传统）** | `.docx`/`.docm`、`.doc`、`.odt`、`.fodt`、`.rtf`、`.wpd`/`.wp`/`.wp5`/`.wp6`、`.hwp`、`.hwpx` | **DOCX**：样式标题、`numbering.xml` 列表、表格（含合并单元格展开）、链接、图片替代文字与部件路径、脚注/尾注、文本框、OMML 数学公式转 LaTeX（页眉页脚、批注与修订删除文字不输出正文）。**DOC**（MS-DOC / Word 6–95）：CLX 片段定位、标题、列表、字段结果、表格、脚注与文本框。**ODT/FODT**、**HWP/HWPX**、**RTF**、**WordPerfect**：标题、段落、列表、表格与标题元数据。 |
| **电子表格** | `.xlsx`/`.xlsm`、`.xlsb`、`.xls`、`.ods`、`.fods` | **XLSX**：共享/行内字符串、日期、数字与透视表数据字段显示格式、跳过隐藏行列、稀疏表非空外接矩形提取、取公式缓存值（不重算公式；正文转换不渲染图表、图片与批注，内嵌图片请使用独立媒体 API `media()`/`media_bytes()`）。**XLSB**（BIFF12）与 **XLS**（BIFF8）：工作表、共享字符串、数值/日期格式与公式缓存值。**ODS/FODS**：工作表及有上限的重复行列展开。 |
| **演示文稿** | `.pptx`/`.pptm`、`.ppt`、`.odp`、`.fodp` | **PPTX**：按演示顺序输出幻灯片、标题、占位符与母版列表项目符号继承、表格、图片替代文字、SmartArt 图示、图表缓存数据与演讲者备注。**PPT**（MS-PPT）：按持久化目录或顺序回退读取正文、列表与备注。**ODP/FODP**：幻灯片正文、列表与表格。 |
| **电子书与结构化 XML** | `.epub`、`.mobi`/`.azw`/`.azw3`/`.prc`、`.fb2`、DocBook、JATS、OPML、SVG | **EPUB**：按 spine 顺序提取章节、Dublin Core 元数据及字体混淆与 DRM 区分；**必须提供 `Options::html` 回调**（未提供时返回 `Error::Unsupported`）。**MOBI/AZW/PRC**、**FB2**、**DocBook**、**JATS**、**OPML**、**SVG**：章节结构、标题、列表、表格、元数据或文字内容。 |
| **邮件** | `.eml`、`.mbox`、`.msg` | RFC 5322/MIME 邮件头与正文、`mbox` 分封解析、Outlook OLE + LZFu RTF `.msg`。存在 HTML 正文时优先调用 `Options::html`，未提供则回退至纯文本正文；附件使用 `Options::member` 递归处理（未提供时仅列附件清单）。 |
| **数据表与数据库** | `.csv`、`.tsv`、`.dbf`、`.sqlite`/`.db`/`.gpkg` | 自动嗅探分隔符的 `.csv`/`.tsv`；无需 `libsqlite3` 读取 `.sqlite`/`.db`/`.gpkg` 模式定义与 B-tree 数据表（含 `WITHOUT ROWID` 与溢出页）。支持**压缩包内 `.dbf` 与配套 `.dbt`/`.fpt` memo 文件**的关联转换（直接对单个 `.dbf` 字节调用公开 `convert()` 时，不会自动读取磁盘同目录 memo 文件）。 |
| **压缩包** | `.zip`、`.tar`、`.gz`/`.tgz`、`.bz2`/`.tbz2`、`.xz`/`.txz`、`.zst`/`.tzst`、`.7z` | 单文件解压与成员清单提取；提供 `Options::member` 时可逐项递归转换内容，未提供时仅列出成员目录。`.zst`/`.tar.zst` 依赖可选 `zstd` feature。 |
| **笔记本、文本与代码** | `.ipynb`、`.srt`/`.vtt`、`.md`、`.txt`、源代码文件 | Jupyter 单元格与输出、SubRip/WebVTT 字幕去重提取、Markdown/纯文本（UTF-8/UTF-16 BOM）及代码块封装（开启 `syntax` feature 后按函数/类等顶层结构切分）。 |

---

## 安装

```toml
[dependencies]
yolowdoc = "0.1"
```

如果仅需部分可选功能（例如不引入 [`yolowsyntax`](https://crates.io/crates/yolowsyntax) 内嵌的语言语法表，仅保留 Zstandard 解压支持）：

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

对于已通过外部探测器或 HTTP 头部解码为 UTF-8 字符串的文本与表格，可直接使用 `convert_text(&text, format, &opts)` 或 `convert_csv_text(&text, &opts)`。

---

## 回调与内嵌媒体 API

- **`Options::html`（`Fn(&str, &str) -> String + Sync`）**：EPUB 转换必填回调（未提供返回 `Error::Unsupported`）；邮件包含 HTML 正文时也通过该回调转换，未提供时回退至纯文本正文。
- **`Options::member`（`Fn(&[u8], &str, usize) -> Result<Option<Converted>, Error> + Sync`）**：用于转换压缩包成员与邮件附件，第 3 个参数传入当前剩余可用输出字节预算；未提供时仅生成成员或附件清单。
- **`media()` 与 `media_bytes()`**：列出并提取 `.docx`、`.pptx`、`.xlsx`、`.epub` 中的内嵌图片及其 SHA-256 哈希和文档位置。

```rust
use yolowdoc::{Converted, Error, Format, Options, convert, media, media_bytes};

// 接入你自己的 HTML 转 Markdown 实现（例如基于 HTML DOM/解析器的转换库）。
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

## Feature 开关

| Feature | 默认 | 说明 |
|---|---|---|
| `syntax` | 开启 | 启用 [`yolowsyntax`](https://crates.io/crates/yolowsyntax) `0.1.2`，对源代码文件按顶层函数/类分段并写入符号和导入元数据；关闭后直接输出带语言标识的代码块，省略内嵌语法表。 |
| `zstd` | 开启 | 启用 [`yolowzstd`](https://crates.io/crates/yolowzstd) `0.1.2`，在共享 `Limits` 配额下解压 `.zst`、`.zstd` 与 `.tar.zst`/`.tzst` 压缩文件。 |

---

## 性能数据与测试条件

### 合成文档基准（`examples/bench.rs`）

- **测试版本**：`yolowdoc 0.1.1`
- **运行环境**：`Intel(R) Core(TM) i7-10700 CPU @ 2.90GHz`（x86_64），`Linux 7.0.0-43-generic`，`rustc 1.99.0`
- **编译与运行条件**：`--release`（`lto = "fat"`、`codegen-units = 1`）、单核绑定（`taskset -c 4`）、预热后采集 25 轮（高耗时大型输入为 5 轮）
- **吞吐计算口径**：按 `input_bytes / 1024^2 / (p50_ms / 1000)` 以完整输入字节折算；进程 `VmHWM` 为整个 benchmark 进程生命周期内的累计峰值，不代表单份文档的独立进程峰值内存。

| 用例 | 工作负载描述 | 输入大小 | 输出大小 | p50 耗时 | p95 耗时 | 吞吐 |
|---|---|---|---|---|---|---|
| `docx_200p` | DOCX 200 段落 | 34,494 B | 25,945 B | 0.170 ms | 0.185 ms | 193.53 MiB/s |
| `docx_2500p` | DOCX 2,500 段落 | 41,454 B | 326,753 B | 1.910 ms | 1.950 ms | 20.70 MiB/s |
| `docx_20000p` | DOCX 20,000 段落 | 95,587 B | 2,631,653 B | 15.825 ms | 16.275 ms | 5.76 MiB/s |
| `xlsx_200x6` | XLSX 200 × 6 单元格 | 8,015 B | 11,017 B | 0.603 ms | 0.632 ms | 12.68 MiB/s |
| `xlsx_4000x8` | XLSX 4,000 × 8 单元格 | 183,400 B | 308,448 B | 15.212 ms | 15.259 ms | 11.50 MiB/s |
| `xlsx_5k_rep_512B` | XLSX 5,000 次引用同一 512 B 共享串 | 254,260 B | 2,585,015 B | 4.240 ms | 4.416 ms | 57.19 MiB/s |
| `xlsx_25000x4` | XLSX 25,000 × 4 单元格 | 603,687 B | 1,176,901 B | 46.726 ms | 60.611 ms | 12.32 MiB/s |
| `pptx_60s` | PPTX 60 页幻灯片 | 21,503 B | 5,123 B | 0.507 ms | 0.594 ms | 40.41 MiB/s |
| `pptx_300s` | PPTX 300 页幻灯片 | 106,209 B | 26,567 B | 2.465 ms | 2.481 ms | 41.09 MiB/s |
| `media_40img` | DOCX 媒体扫描（40 × 16 KiB PNG） | 668,676 B | 40 items | 2.720 ms | 2.752 ms | 234.43 MiB/s |
| `csv_10000r` | CSV 10,000 行 × 5 列 | 580,399 B | 680,443 B | 2.094 ms | 2.141 ms | 264.36 MiB/s |
| `csv_1M_rows` | CSV 百万行输入，预算限制下的转换（`max_output_bytes = 16 MiB`、`max_cells = 2,000,000`，每行 5 列） | 64,037,065 B | 16,777,204 B（已截断） | 97.313 ms | 98.006 ms | 627.57 MiB/s* |
| `sqlite_4800r` | SQLite 4,800 行（内部页 + 叶子 B-tree 页） | 499,712 B | 283,258 B | 1.782 ms | 1.839 ms | 267.47 MiB/s |
| `zip_200f` | ZIP 200 个文本条目 | 22,312 B | 6,645 B | 0.075 ms | 0.078 ms | 284.05 MiB/s |
| `tar_200f` | TAR 200 个文本条目 | 205,824 B | 6,645 B | 0.064 ms | 0.069 ms | 3,059.23 MiB/s |
| `nested_archive_400f` | 嵌套 ZIP + TAR 成员清单（400 个条目；回调未向内层传递 `Options::member`） | 228,346 B | 13,438 B | 0.270 ms | 0.289 ms | 807.08 MiB/s |

\* `csv_1M_rows` 在完成全部百万行输出前即触达默认 `max_output_bytes`（`16 MiB`）预算上限；上表吞吐按完整输入文件大小除以 `p50` 耗时折算，不代表完整处理并输出全部百万行的实际吞吐。

延迟越低越好，吞吐越高越好。在热点微基准归因（`perf_micro_attribution`，`0.1.1` 对比 `0.1.0`）中，`xlsx_4000x8` 分配次数由 `145,131` 次降至 `1,137` 次，中位耗时由 `18.856 ms` 降至 `15.212 ms`。

---

## 测试与可复现性

项目内已建立多层次验证体系：

- **单元测试与集成测试**：各容器与格式模块内单元测试（默认 feature 下共 171 项），以及覆盖公开接口、回调与预算控制的集成测试（[`tests/integration.rs`](../tests/integration.rs)）和文档测试。
- **真实文档语料清单**：[`tests/corpora.tsv`](../tests/corpora.tsv) 记录 **72** 份经 SHA-256 校验的公开真实文档（`20` 份 DOCX、`16` 份 XLSX、`16` 份 PPTX、`16` 份 CSV、`4` 份 EPUB）及其来源 URL、许可证、字节数与运行输出大小快照（其中的 `out_bytes` 与 `warnings` 属于输出记录，不作为人工标注结构准确率）。
- **内部解析器变异 Fuzz**：[`src/fuzz_tests.rs`](../src/fuzz_tests.rs)（`#[ignore]`）绕过公开入口异常捕获，直接对 24 个内部解析目标执行长度、偏移、压缩与结构变异测试。
- **微基准与端到端基准**：[`src/bench_internal.rs`](../src/bench_internal.rs)（`#[ignore]`）记录分配次数、累计与峰值堆字节及热点延迟；[`examples/bench.rs`](../examples/bench.rs) 测量完整文档吞吐。
- **跨平台 CI 与 Feature 矩阵**：[`ci.yml`](../.github/workflows/ci.yml) 在 `ubuntu-latest`（Rust `1.89.0` 与 `stable`）、`macos-latest`（`stable`）和 `windows-latest`（`stable`）上运行 `fmt`、`clippy`、debug 测试以及 default、`--no-default-features`、`--features syntax`、`--features zstd` 四种组合。

```bash
# 运行标准测试与四种 feature 组合
cargo test --release --locked
cargo test --release --locked --no-default-features
cargo test --release --locked --no-default-features --features syntax
cargo test --release --locked --no-default-features --features zstd

# 复现性能基准与内部变异 Fuzz
cargo run --release --example bench
cargo test --release fuzz_mutating -- --ignored --nocapture
cargo test --release perf_micro_attribution -- --ignored --nocapture
```

---

## 资源限制与格式边界

可通过 `Options::limits`（`Limits` 结构体）配置的上限：

| 字段 | 默认值 | 说明与作用范围 |
|---|---|---|
| `max_zip_entries` | `20_000` | ZIP 中央目录允许的最大条目数。 |
| `max_entry_bytes` | `256 MiB` | 单个 ZIP 条目、压缩文件或文档部件允许解压出的最大字节数。 |
| `max_total_bytes` | `512 MiB` | 单次文档转换期间累计读取解压字节数的上限。 |
| `max_ratio` | `400` | 仅对 ZIP 中解压后大于 `4 MiB` 的大条目生效的压缩比上限，并非面向所有压缩格式的统一压缩比限制。 |
| `max_cells` | `2_000_000` | 电子表格与 CSV 最大输出单元格数量；超出部分行截断并附带警告。 |
| `max_columns` | `1_024` | 电子表格与 SQLite 单表最大列数（OOXML 规范上限为 16,384）；超出列丢弃并附带警告。 |
| `max_output_bytes` | `16 MiB` | 返回 Markdown 文本的累计字节预算；超出时返回已转换内容并附带 `output_truncated` 警告。 |

其他运行边界说明：

- **固定内部限制**：压缩包与邮件附件的最大递归嵌套深度固定为 **3 层**；归档容器和 mbox 共用的单次最高成员/邮件处理数限制为 **1,000**。
- **协作式截止时间（`Options::deadline`）**：在段落、行、幻灯片、章节、成员及媒体读取边界处检查时间点，超时返回 `Error::Timeout`，属于协作式检查而非抢占式强制终止。
- **输出预算不等于进程内存隔离**：`max_output_bytes` 仅限制输出 Markdown 长度并帮助提前停止后续转换，不构成对进程总 RSS 或虚拟内存的硬上限保证，也不承诺任意恶意输入下的绝对执行时间上界。

---

## 协议

本项目采用 [MIT License](../LICENSE) 开源。
