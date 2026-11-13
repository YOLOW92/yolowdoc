//! 工作表（worksheet，ECMA-376 18.3）：流式读单元格，只留有内容的，最后排成表。

use super::Book;
use super::pivot::{Area, format_at};
use crate::md::Grid;
use crate::xml::{Reader, Tok, local_name};
use crate::{Error, Options, budget};

/// 表里连续这么多个空行以上就不再原样输出（稀疏的大表）。
const MAX_EMPTY_RUN: u32 = 1000;

#[derive(Clone, Copy)]
enum CellVal {
    Shared(u32),
    Range(u32, u32),
    Bool(bool),
}

impl CellVal {
    fn as_str<'a>(self, shared: &'a [String], owned: &'a str) -> &'a str {
        match self {
            CellVal::Shared(i) => shared.get(i as usize).map_or("", String::as_str),
            CellVal::Range(a, b) => &owned[a as usize..b as usize],
            CellVal::Bool(true) => "TRUE",
            CellVal::Bool(false) => "FALSE",
        }
    }
}

/// 读一个工作表，返回表格（第一行是表头）。`budget` 是剩下可输出的单元格数，用完就截断并警告。
pub(super) fn read(
    xml: &str,
    book: &Book,
    pivots: &[Area],
    opts: &Options<'_>,
    budget: &mut usize,
    warnings: &mut Vec<String>,
) -> Result<Grid, Error> {
    let max_cols = opts.limits.max_columns as u32;
    let mut cells: Vec<(u32, u32, CellVal)> = Vec::new();
    let mut owned_text = String::new();
    let mut v_buf = String::new();
    let mut inline_buf = String::new();
    let mut row = 0u32;
    let mut col = 0u32;
    let mut dropped_cols = false;
    let mut truncated = false;
    // 隐藏的列（cols/col@hidden）和行（row@hidden）：用户在表格软件里看不到，不输出
    let mut hidden_cols: Vec<(u32, u32)> = Vec::new();
    let mut hidden_row = false;
    let mut hidden_rows: Vec<u32> = Vec::new();
    let mut unbilled = 0usize;
    let mut r = Reader::new(xml);
    while let Some(t) = r.next()? {
        let Tok::Start(name, a) = t else { continue };
        match local_name(name) {
            "col" => {
                if flag(a.get("hidden").as_deref())
                    && let (Some(lo), Some(hi)) = (
                        a.get("min").and_then(|v| v.parse().ok()),
                        a.get("max").and_then(|v| v.parse().ok()),
                    )
                {
                    hidden_cols.push((lo, hi));
                }
            }
            "row" => {
                row = a.get("r").and_then(|v| v.parse().ok()).unwrap_or(row + 1);
                col = 0;
                hidden_row = flag(a.get("hidden").as_deref());
                if hidden_row {
                    hidden_rows.push(row);
                }
                if row.is_multiple_of(4096) {
                    crate::check_deadline(opts)?;
                }
            }
            "c" => {
                #[derive(Clone, Copy)]
                enum CellKind {
                    Num,
                    Shared,
                    InlineStr,
                    Bool,
                    Text,
                }
                let mut pos = None;
                let mut kind = CellKind::Num;
                let mut style: usize = 0;
                for (k, raw_val) in a.raw() {
                    match local_name(k) {
                        "r" => {
                            let val = crate::xml::unescape(raw_val);
                            pos = cell_ref(&val);
                        }
                        "t" => {
                            let val = crate::xml::unescape(raw_val);
                            kind = match val.as_ref() {
                                "s" => CellKind::Shared,
                                "inlineStr" => CellKind::InlineStr,
                                "b" => CellKind::Bool,
                                "e" | "str" | "d" => CellKind::Text,
                                _ => CellKind::Num,
                            };
                        }
                        "s" => {
                            let val = crate::xml::unescape(raw_val);
                            style = val.parse().unwrap_or(0);
                        }
                        _ => {}
                    }
                }
                let (rr, cc) = pos.unwrap_or((row, col + 1));
                row = rr;
                col = cc;
                cell_content_into(&mut r, &mut v_buf, &mut inline_buf)?;
                if hidden_row || hidden_cols.iter().any(|&(lo, hi)| (lo..=hi).contains(&cc)) {
                    continue;
                }
                if cc > max_cols {
                    dropped_cols = true;
                    continue;
                }
                let (val, text_len) = match kind {
                    CellKind::Shared => {
                        let Some(s_val) = v_buf
                            .trim()
                            .parse::<u32>()
                            .ok()
                            .and_then(|i| book.shared.get(i as usize).map(|s| (i, s)))
                        else {
                            continue;
                        };
                        if s_val.1.trim().is_empty() {
                            continue;
                        }
                        (CellVal::Shared(s_val.0), s_val.1.len())
                    }
                    CellKind::InlineStr => {
                        if inline_buf.trim().is_empty() {
                            continue;
                        }
                        let start = owned_text.len() as u32;
                        owned_text.push_str(&inline_buf);
                        let end = owned_text.len() as u32;
                        (CellVal::Range(start, end), (end - start) as usize)
                    }
                    CellKind::Bool => {
                        let b = v_buf.trim() == "1";
                        (CellVal::Bool(b), if b { 4 } else { 5 })
                    }
                    CellKind::Text => {
                        if v_buf.trim().is_empty() {
                            continue;
                        }
                        let start = owned_text.len() as u32;
                        owned_text.push_str(&v_buf);
                        let end = owned_text.len() as u32;
                        (CellVal::Range(start, end), (end - start) as usize)
                    }
                    CellKind::Num if v_buf.is_empty() => continue,
                    CellKind::Num => {
                        let start = owned_text.len();
                        book.number_into(&v_buf, style, format_at(pivots, rr, cc), &mut owned_text);
                        if owned_text[start..].trim().is_empty() {
                            owned_text.truncate(start);
                            continue;
                        }
                        let end = owned_text.len();
                        (CellVal::Range(start as u32, end as u32), end - start)
                    }
                };
                if cells.len() >= *budget {
                    truncated = true;
                    break;
                }
                unbilled += text_len;
                cells.push((rr, cc, val));
                if unbilled >= budget::BATCH {
                    let ok = budget::spend(unbilled);
                    unbilled = 0;
                    if !ok {
                        break;
                    }
                }
            }
            "mergeCells" | "conditionalFormatting" | "dataValidations" | "extLst" => r.skip()?,
            _ => {}
        }
    }
    if unbilled > 0 {
        budget::spend(unbilled);
    }
    if dropped_cols {
        warnings.push(format!(
            "columns_dropped: columns beyond {max_cols} were dropped"
        ));
    }
    if cells.is_empty() {
        return Ok(Grid::default());
    }
    // 去掉隐藏的行列后，后面的往前挪，表里不留空行空列
    if !hidden_cols.is_empty() || !hidden_rows.is_empty() {
        hidden_rows.sort_unstable();
        for c in &mut cells {
            let shift: u32 = hidden_cols
                .iter()
                .map(|&(lo, hi)| if hi < c.1 { hi - lo + 1 } else { 0 })
                .sum();
            c.1 -= shift.min(c.1 - 1);
            c.0 -= hidden_rows.partition_point(|&r| r < c.0) as u32;
        }
    }
    let c0 = cells.iter().map(|c| c.1).min().unwrap_or(1);
    let c1 = cells.iter().map(|c| c.1).max().unwrap_or(1);
    let width = (c1 - c0 + 1) as usize;
    cells.sort_by_key(|c| (c.0, c.1));

    let mut grid = Grid::default();
    let mut last_row = 0u32;
    let mut skipped = 0u64;
    let mut cut = false;
    let mut idx = 0usize;
    while idx < cells.len() {
        let rr = cells[idx].0;
        let mut row_end = idx + 1;
        while row_end < cells.len() && cells[row_end].0 == rr {
            row_end += 1;
        }
        if grid.rows() > 0 {
            let gap = rr - last_row - 1;
            if gap > MAX_EMPTY_RUN {
                skipped += u64::from(gap);
            } else {
                for _ in 0..gap {
                    if *budget < width {
                        cut = true;
                        break;
                    }
                    *budget -= width;
                    for _ in 0..width {
                        grid.push_cell("");
                    }
                    grid.end_row();
                }
            }
        }
        if cut || *budget < width {
            cut = true;
            break;
        }
        *budget -= width;
        let mut cur_c = c0;
        let mut j = idx;
        while j < row_end {
            let cc = cells[j].1;
            // 同坐标覆盖：保留后出现的单元格
            while j + 1 < row_end && cells[j + 1].1 == cc {
                j += 1;
            }
            while cur_c < cc {
                grid.push_cell("");
                cur_c += 1;
            }
            grid.push_cell(cells[j].2.as_str(&book.shared, &owned_text));
            cur_c = cc + 1;
            j += 1;
        }
        while cur_c <= c1 {
            grid.push_cell("");
            cur_c += 1;
        }
        grid.end_row();
        last_row = rr;
        idx = row_end;
    }
    if skipped > 0 {
        warnings.push(format!(
            "skipped {skipped} empty rows inside a sparse sheet"
        ));
    }
    if truncated || cut {
        warnings.push(format!(
            "cells_truncated: sheet truncated at the cell limit ({} cells)",
            opts.limits.max_cells
        ));
    }
    Ok(grid)
}

fn flag(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true"))
}

/// 单元格里的值 `v` 和行内字符串 `is`（刚读到 `c` 的开始标签），复用调用方缓冲区。公式 `f` 不要。
fn cell_content_into(r: &mut Reader<'_>, v: &mut String, inline: &mut String) -> Result<(), Error> {
    v.clear();
    inline.clear();
    let mut depth = 1;
    while let Some(t) = r.next()? {
        match t {
            Tok::Start(n, _) => match local_name(n) {
                "v" => {
                    v.clear();
                    r.text_until_end_into(v)?;
                }
                "t" => r.text_until_end_into(inline)?,
                "f" | "rPh" | "extLst" => r.skip()?,
                _ => depth += 1,
            },
            Tok::End(_) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Tok::Text(_) => {}
        }
    }
    Ok(())
}

/// `AB12` → (行 12, 列 28)。
pub(super) fn cell_ref(s: &str) -> Option<(u32, u32)> {
    let letters = s.bytes().take_while(u8::is_ascii_alphabetic).count();
    if letters == 0 || letters > 3 {
        return None;
    }
    let col = s[..letters].bytes().fold(0u32, |n, b| {
        n * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    let row = s[letters..].parse().ok()?;
    Some((row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cell_references() {
        assert_eq!(cell_ref("A1"), Some((1, 1)));
        assert_eq!(cell_ref("AB12"), Some((12, 28)));
        assert_eq!(cell_ref("XFD1048576"), Some((1_048_576, 16_384)));
        assert_eq!(cell_ref("12"), None);
    }
}
