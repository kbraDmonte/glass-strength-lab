//! Ring sizes, test areas and the conversion to a common test area.
//!
//! At equal probability of failure the two-parameter Weibull distribution gives
//!   σ_A1 = σ_A0 · (A0 / A1)^(1/m)
//! so the line on the Weibull paper shifts in parallel and m stays the same.
//! Test area = area under quasi-uniform stress: the circle inside the load ring
//! (π·r²) for DIN EN 1288-5 and any other ring size, 240 000 mm² for
//! DIN EN 1288-2 (ring 600 : 800) as specified by that standard.

use crate::text::g_format;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

/// (load-ring diameter, support-ring diameter) in mm, as written e.g. "40:80"
/// or "55 / 145".
pub fn parse_ring(ring: &str) -> Option<(f64, f64)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(\d+(?:[.,]\d+)?)\s*[/:]\s*(\d+(?:[.,]\d+)?)").unwrap());
    let c = re.captures(ring)?;
    let n = |i: usize| c[i].replace(',', ".").parse::<f64>().ok();
    Some((n(1)?, n(2)?))
}

/// Uniform spelling "40 : 80"; unreadable values stay as they are.
pub fn ring_text(ring: &str) -> String {
    match parse_ring(ring) {
        Some((a, b)) => format!("{} : {}", g_format(a), g_format(b)),
        None => ring.to_string(),
    }
}

/// Test area in mm² for a ring size; None if unreadable.
pub fn ring_area(ring: &str) -> Option<f64> {
    let (d1, d2) = parse_ring(ring)?;
    if d1 <= 0.0 {
        return None;
    }
    if (d1, d2) == (600.0, 800.0) {
        return Some(240_000.0);
    }
    Some(std::f64::consts::PI * (d1 / 2.0).powi(2))
}

pub const STANDARD_TARGETS: [((u32, u32), &str); 3] =
    [((600, 800), "DIN EN 1288-2"), ((18, 90), "DIN EN 1288-5 R45"), ((12, 60), "DIN EN 1288-5 R30")];

/// Selectable target areas: the standard set-ups and every ring size in the
/// data, largest first.
pub fn target_areas<'a>(rings: impl Iterator<Item = &'a str>) -> Vec<Value> {
    let mut out: Vec<(f64, Value)> = Vec::new();
    let mut seen: Vec<i64> = Vec::new();
    for ((d1, d2), norm) in STANDARD_TARGETS {
        let ring = format!("{d1}/{d2}");
        let a = ring_area(&ring).unwrap();
        seen.push((a * 10.0).round() as i64);
        out.push((a, json!({"mm2": a, "ring": ring_text(&ring), "norm": norm})));
    }
    let mut rings: Vec<&str> = rings.collect();
    rings.sort();
    rings.dedup();
    for ring in rings {
        if let Some(a) = ring_area(ring) {
            let key = (a * 10.0).round() as i64;
            if !seen.contains(&key) {
                seen.push(key);
                out.push((a, json!({"mm2": a, "ring": ring_text(ring), "norm": Value::Null})));
            }
        }
    }
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    out.into_iter().map(|(_, v)| v).collect()
}
