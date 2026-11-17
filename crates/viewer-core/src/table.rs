//! The measurements behind the series as a plain table (all columns of the
//! Excel sheet), for viewing in the page and for copying to Excel.

use crate::data::{series_rows, Data};
use crate::results::series_name;
use serde_json::{json, Value};

/// `{"spalten": [...columns, strength column], "serien": [{"name", "zeilen"}]}`.
/// Rows keep the order of the file; the strength is the measured (unconverted)
/// value.
pub fn rows_table(data: &Data, req: &Value) -> Value {
    let mut columns = data.header.clone();
    columns.push(data.strength_column.clone());
    let series = req.get("serien").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let out: Vec<Value> = series.iter().map(|s| {
        let mut rows = series_rows(data, s);
        rows.sort_unstable();
        let rows: Vec<Value> = rows.iter().map(|&r| {
            let row = &data.rows[r];
            let mut cells: Vec<Value> = row.cells.iter().map(|c| json!(c)).collect();
            cells.push(json!(row.sigma));
            Value::Array(cells)
        }).collect();
        json!({"name": series_name(s), "zeilen": rows})
    }).collect();
    json!({"spalten": columns, "serien": out})
}
