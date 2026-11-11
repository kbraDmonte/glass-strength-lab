//! Filter columns, measurement rows and series.

use crate::area::{ring_area, ring_text, target_areas};
use crate::excel::{ExcelFile, Measurement};
use crate::text::{display, natural_cmp, normalize, to_number};
use crate::Cell;
use glass_stats::{fit_weibull, PlottingPositions, Settings, WeibullMethod};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

/// Synthetic column that shows the origin file when several files are loaded.
pub const FILE_COLUMN: &str = "Excel-Datei";
/// Never offered as filters (running numbers).
pub const NEVER_FILTER: [&str; 4] = ["Measurement number", "Messwertnummer", "Sample number", "Probennummer"];
/// Numeric columns with more distinct values are no use as filters
/// (thicknesses, forces); text columns such as "ISO number" may have more.
pub const MAX_FILTER_VALUES_NUMBER: usize = 40;
pub const MAX_FILTER_VALUES_TEXT: usize = 200;
/// Column names (lower case) that hold the ring size.
pub const RING_COLUMNS: [&str; 3] = ["ring size", "ringgröße", "ringgroesse"];
/// Conversion needs at least this many specimens per ring size to fit m …
pub const MIN_N_AREA: usize = 3;
/// … and warns below this many (m and thus the factor are uncertain).
pub const FEW_N_AREA: usize = 10;

#[derive(Clone, Debug)]
pub struct Row {
    pub sigma: f64,
    /// filter key per filter column (same order as `Data::columns`)
    pub keys: Vec<String>,
    /// display text per filter column
    pub display: Vec<String>,
    /// ring size as displayed ("–" if missing), None without a ring column
    pub ring: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ColumnValue {
    pub key: String,
    pub display: String,
    pub n: usize,
}

#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    pub values: Vec<ColumnValue>,
}

#[derive(Clone, Debug)]
pub struct Data {
    pub files: Vec<(String, String, usize, String)>, // name, sheet, n, hash
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub ring_column: Option<String>,
}

impl Data {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }
}

pub fn build_data(files: &[ExcelFile]) -> Data {
    let several = files.len() > 1;
    let mut header: Vec<String> = if several { vec![FILE_COLUMN.into()] } else { vec![] };
    for f in files {
        for k in &f.header {
            if !header.contains(k) {
                header.push(k.clone());
            }
        }
    }
    let ring_column = header.iter().find(|k| RING_COLUMNS.contains(&k.trim().to_lowercase().as_str())).cloned();
    // measurements with normalised ring size (and the file column if needed)
    let mut ms: Vec<Measurement> = Vec::new();
    for f in files {
        for m in &f.measurements {
            let mut m = m.clone();
            if several {
                m.cells.push((FILE_COLUMN.into(), Cell::Text(f.file_name.clone())));
            }
            if let Some(rc) = &ring_column {
                for (k, c) in m.cells.iter_mut() {
                    if k == rc && *c != Cell::Empty {
                        if let Some(t) = crate::text::cell_text(c) {
                            let neu = ring_text(&t);
                            if neu != t {
                                *c = Cell::Text(neu);
                            }
                        }
                    }
                }
            }
            ms.push(m);
        }
    }
    // filter columns: everything except running numbers and columns with too
    // many distinct values
    let mut filter_columns = Vec::new();
    for name in &header {
        if NEVER_FILTER.contains(&name.as_str()) {
            continue;
        }
        let values: Vec<&Cell> = ms.iter().map(|m| m.get(name)).filter(|c| **c != Cell::Empty).collect();
        let distinct: HashSet<String> = values.iter().map(|c| normalize(c)).collect();
        let numbers = values.iter().filter(|c| to_number(c).is_some()).count();
        let limit = if numbers as f64 > 0.5 * values.len() as f64 { MAX_FILTER_VALUES_NUMBER } else { MAX_FILTER_VALUES_TEXT };
        if distinct.len() > 1 && distinct.len() <= limit {
            filter_columns.push(name.clone());
        }
    }
    let mut buckets: Vec<(Vec<String>, HashMap<String, (String, usize)>)> =
        filter_columns.iter().map(|_| (Vec::new(), HashMap::new())).collect();
    let mut rows = Vec::with_capacity(ms.len());
    for m in &ms {
        let mut keys = Vec::with_capacity(filter_columns.len());
        let mut disp = Vec::with_capacity(filter_columns.len());
        for (j, name) in filter_columns.iter().enumerate() {
            let c = m.get(name);
            let (k, d) = (normalize(c), display(c));
            let (order, map) = &mut buckets[j];
            let e = map.entry(k.clone()).or_insert_with(|| { order.push(k.clone()); (d.clone(), 0) });
            e.1 += 1;
            keys.push(k);
            disp.push(d);
        }
        let ring = ring_column.as_ref().map(|rc| display(m.get(rc)));
        rows.push(Row { sigma: m.sigma, keys, display: disp, ring });
    }
    let columns = filter_columns.iter().zip(buckets).map(|(name, (order, map))| {
        let mut values: Vec<ColumnValue> = order.iter()
            .map(|k| ColumnValue { key: k.clone(), display: map[k].0.clone(), n: map[k].1 }).collect();
        values.sort_by(|a, b| natural_cmp(&a.display, &b.display));
        Column { name: name.clone(), values }
    }).collect();
    Data {
        files: files.iter().map(|f| (f.file_name.clone(), f.sheet.clone(), f.measurements.len(), f.hash.clone())).collect(),
        columns, rows, ring_column,
    }
}

/// The parts of the page's start-up data that depend on the loaded files.
pub fn data_response(data: &Data) -> Value {
    json!({
        "dateien": data.files.iter().map(|(n, s, k, _)| json!({"datei": n, "blatt": s, "n": k})).collect::<Vec<_>>(),
        "spalten": data.columns.iter().map(|c| json!({"name": c.name, "werte": c.values.iter()
            .map(|v| json!({"schluessel": v.key, "anzeige": v.display, "n": v.n})).collect::<Vec<_>>()}))
            .collect::<Vec<_>>(),
        "zeilen": data.rows.iter().map(|r| json!(r.keys)).collect::<Vec<_>>(),
        "zielflaechen": target_areas(data.rows.iter().filter_map(|r| r.ring.as_deref())),
        "ring_spalte": data.ring_column,
    })
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map(crate::text::number_text).unwrap_or_default(),
        Value::Bool(b) => if *b { "True".into() } else { "False".into() },
        _ => String::new(),
    }
}

/// Rows that meet all conditions: AND across columns, OR within a column.
pub fn select_rows<'a>(data: &'a Data, filter: Option<&Map<String, Value>>) -> Vec<usize> {
    let mut conditions: Vec<(Option<usize>, HashSet<String>)> = Vec::new();
    if let Some(f) = filter {
        for (col, allowed) in f {
            let Some(list) = allowed.as_array() else { continue };
            if list.is_empty() {
                continue;
            }
            let ring = data.ring_column.as_deref() == Some(col.as_str());
            let set = list.iter().map(|v| {
                let t = value_text(v);
                let t = if ring { ring_text(&t) } else { t };
                t.trim().to_lowercase()
            }).collect();
            conditions.push((data.column_index(col), set));
        }
    }
    (0..data.rows.len()).filter(|&i| conditions.iter().all(|(j, set)| match j {
        Some(j) => set.contains(&data.rows[i].keys[*j]),
        None => false,
    })).collect()
}

/// Rows of a series; a combined series ("vereinigung": list of filters)
/// contains each row that matches one of its filters, once.
pub fn series_rows(data: &Data, series: &Value) -> Vec<usize> {
    match series.get("vereinigung").and_then(|v| v.as_array()).filter(|v| !v.is_empty()) {
        None => select_rows(data, series.get("filter").and_then(|f| f.as_object())),
        Some(parts) => {
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for p in parts {
                for i in select_rows(data, p.as_object()) {
                    if seen.insert(i) {
                        out.push(i);
                    }
                }
            }
            out
        }
    }
}

#[derive(Clone, Debug)]
pub struct SeriesValues {
    pub rows: Vec<usize>,
    pub sigma: Vec<f64>,
    pub sigma_orig: Vec<f64>,
    pub factors: Vec<f64>,
    pub conversion: Option<Value>,
    pub hints: Vec<Value>,
}

/// Factor per measurement for the conversion to the target area; m per ring
/// size from its own Weibull fit. Returns (factors or None, groups, hints);
/// with None the series stays unconverted — never partly converted.
pub fn convert_area(data: &Data, rows: &[usize], sigma: &[f64], target_mm2: f64,
                    method: WeibullMethod, positions: PlottingPositions) -> (Option<Vec<f64>>, Vec<Value>, Vec<Value>) {
    if data.ring_column.is_none() {
        return (None, vec![], vec![json!({"code": "umr_keine_spalte"})]);
    }
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, &r) in rows.iter().enumerate() {
        let ring = data.rows[r].ring.clone().unwrap_or_else(|| "–".into());
        groups.entry(ring.clone()).or_insert_with(|| { order.push(ring.clone()); Vec::new() }).push(i);
    }
    // largest group first (stable for equal sizes)
    let mut sorted: Vec<&String> = order.iter().collect();
    sorted.sort_by(|a, b| groups[*b].len().cmp(&groups[*a].len()));
    let mut factors = vec![1.0; rows.len()];
    let (mut info, mut hints) = (Vec::new(), Vec::new());
    let settings = Settings { method, positions, ..Settings::default() };
    for ring in sorted {
        let idx = &groups[ring];
        let Some(area) = ring_area(ring) else {
            return (None, vec![], vec![json!({"code": "umr_ring_unbekannt", "ring": ring})]);
        };
        if idx.len() < MIN_N_AREA {
            return (None, vec![], vec![json!({"code": "umr_zu_wenig", "ring": ring_text(ring), "n": idx.len()})]);
        }
        let mut s: Vec<f64> = idx.iter().map(|&i| sigma[i]).collect();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let m = fit_weibull(&s, &settings).map(|f| f.shape).unwrap_or(f64::NAN);
        if !m.is_finite() || m <= 0.0 {
            return (None, vec![], vec![json!({"code": "umr_m", "ring": ring_text(ring)})]);
        }
        let factor = (area / target_mm2).powf(1.0 / m);
        for &i in idx {
            factors[i] = factor;
        }
        info.push(json!({"ring": ring_text(ring), "n": idx.len(), "flaeche_mm2": area, "m": m, "faktor": factor}));
        if idx.len() < FEW_N_AREA {
            hints.push(json!({"code": "umr_wenig", "ring": ring_text(ring), "n": idx.len()}));
        }
    }
    (Some(factors), info, hints)
}

/// Measurements of a series, sorted ascending and — with a target area —
/// converted per ring size. Common basis of every evaluation.
pub fn series_values(data: &Data, series: &Value, target_mm2: f64, method: WeibullMethod,
                     positions: PlottingPositions) -> SeriesValues {
    let mut rows = series_rows(data, series);
    rows.sort_by(|a, b| data.rows[*a].sigma.partial_cmp(&data.rows[*b].sigma).unwrap());
    let sigma: Vec<f64> = rows.iter().map(|&r| data.rows[r].sigma).collect();
    let mut factors = vec![1.0; rows.len()];
    let (mut conversion, mut hints) = (None, vec![]);
    if target_mm2 > 0.0 && rows.len() >= 3 {
        let (f, info, h) = convert_area(data, &rows, &sigma, target_mm2, method, positions);
        hints = h;
        if let Some(f) = f {
            factors = f;
            conversion = Some(json!({"ziel_mm2": target_mm2, "gruppen": info}));
        }
    }
    // re-sort the converted values (with several ring sizes the order can change)
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| (sigma[a] * factors[a]).partial_cmp(&(sigma[b] * factors[b])).unwrap());
    let sigma_orig: Vec<f64> = order.iter().map(|&i| sigma[i]).collect();
    let factors: Vec<f64> = order.iter().map(|&i| factors[i]).collect();
    SeriesValues {
        rows: order.iter().map(|&i| rows[i]).collect(),
        sigma: sigma_orig.iter().zip(&factors).map(|(s, f)| s * f).collect(),
        sigma_orig, factors, conversion, hints,
    }
}
