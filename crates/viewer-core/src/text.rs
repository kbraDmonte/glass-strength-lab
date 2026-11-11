//! Turning cell values into filter keys, display texts and numbers.

use crate::Cell;
use std::cmp::Ordering;

/// Number as text: integral values without decimals, others in the shortest
/// form that reads back to the same number.
pub fn number_text(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e16 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// Cell as plain text (what the cell shows), no trimming.
pub fn cell_text(c: &Cell) -> Option<String> {
    match c {
        Cell::Empty => None,
        Cell::Number(v) => Some(number_text(*v)),
        Cell::Text(s) => Some(s.clone()),
        Cell::Bool(b) => Some(if *b { "True".into() } else { "False".into() }),
    }
}

/// Filter key: trimmed, lower case; empty for empty cells.
pub fn normalize(c: &Cell) -> String {
    cell_text(c).map(|s| s.trim().to_lowercase()).unwrap_or_default()
}

/// Display text; "–" for empty cells.
pub fn display(c: &Cell) -> String {
    match cell_text(c) {
        None => "–".into(),
        Some(s) if s.trim().is_empty() => "–".into(),
        Some(s) => s.trim().to_string(),
    }
}

/// Number from a cell: numbers as they are, text with a decimal comma or point.
pub fn to_number(c: &Cell) -> Option<f64> {
    match c {
        Cell::Number(v) => Some(*v),
        Cell::Text(s) => s.trim().replace(',', ".").parse::<f64>().ok(),
        _ => None,
    }
}

/// Python-style "g" format (6 significant digits, no trailing zeros).
pub fn g_format(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let exp = v.abs().log10().floor() as i32;
    // rounding to 6 digits can move the exponent (e.g. 999999.7)
    let rounded: f64 = format!("{:.5e}", v).parse().unwrap();
    let exp = if rounded.abs() >= 10f64.powi(exp + 1) { exp + 1 } else { exp };
    if (-4..6).contains(&exp) {
        let decimals = (5 - exp).max(0) as usize;
        let s = format!("{:.*}", decimals, v);
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
    } else {
        let s = format!("{:.5e}", v);
        let (mant, e) = s.split_once('e').unwrap();
        let mant = if mant.contains('.') { mant.trim_end_matches('0').trim_end_matches('.') } else { mant };
        let e: i32 = e.parse().unwrap();
        format!("{}e{}{:02}", mant, if e < 0 { '-' } else { '+' }, e.abs())
    }
}

/// Natural sort order: "G1-9" before "G1-17", numbers numerically.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn parts(s: &str) -> Vec<(u8, String)> {
        let mut out = Vec::new();
        let mut digits = String::new();
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                digits.push(ch);
                continue;
            }
            if !digits.is_empty() {
                out.push((1, std::mem::take(&mut digits)));
            }
            out.push((2, ch.to_lowercase().collect()));
        }
        if !digits.is_empty() {
            out.push((1, digits));
        }
        out
    }
    fn num_cmp(x: &str, y: &str) -> Ordering {
        let x = x.trim_start_matches('0');
        let y = y.trim_start_matches('0');
        x.len().cmp(&y.len()).then_with(|| x.cmp(y))
    }
    let (pa, pb) = (parts(a), parts(b));
    for (x, y) in pa.iter().zip(pb.iter()) {
        let o = x.0.cmp(&y.0).then_with(|| if x.0 == 1 { num_cmp(&x.1, &y.1) } else { x.1.cmp(&y.1) });
        if o != Ordering::Equal {
            return o;
        }
    }
    pa.len().cmp(&pb.len())
}
