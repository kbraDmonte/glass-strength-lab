//! Reading the data sheet of an Excel file.
//!
//! Rules (unchanged from the previous viewer):
//! - the sheet "Dataset" is used, otherwise "Datensatz";
//! - the first row holds the column names;
//! - the strength column is the first one whose name contains
//!   "fracture stress" or "Bruchspannung" (case-insensitive);
//! - a row is skipped if its first cell is empty or reads "hide"/"ausblenden",
//!   or if its strength is missing, not a number, zero or negative.

use crate::text::{cell_text, normalize, to_number};
use calamine::{open_workbook_from_rs, Data as XData, Reader, Xlsx};
use serde_json::{json, Map, Value};
use std::io::Cursor;

pub const SHEET_NAMES: [&str; 2] = ["Dataset", "Datensatz"];
pub const STRENGTH_COLUMN_CONTAINS: [&str; 2] = ["fracture stress", "bruchspannung"];

#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
}

impl From<&XData> for Cell {
    fn from(d: &XData) -> Self {
        match d {
            XData::Empty => Cell::Empty,
            XData::Int(i) => Cell::Number(*i as f64),
            XData::Float(f) => Cell::Number(*f),
            XData::String(s) => Cell::Text(s.clone()),
            XData::Bool(b) => Cell::Bool(*b),
            XData::DateTime(d) => Cell::Number(d.as_f64()),
            XData::DateTimeIso(s) | XData::DurationIso(s) => Cell::Text(s.clone()),
            XData::Error(e) => Cell::Text(e.to_string()),
        }
    }
}

/// Error while reading a file. `code` and `details` go to the page, which
/// builds the message in the chosen language.
#[derive(Clone, Debug, PartialEq)]
pub struct DataError {
    pub code: String,
    pub details: Map<String, Value>,
}

impl DataError {
    pub fn new(code: &str) -> Self {
        DataError { code: code.into(), details: Map::new() }
    }
    pub fn with(mut self, key: &str, value: Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }
    pub fn to_json(&self) -> Value {
        json!({"fehler_code": self.code, "details": self.details})
    }
}

/// One measurement: the strength and the other cells of its row by column name.
#[derive(Clone, Debug)]
pub struct Measurement {
    pub sigma: f64,
    pub cells: Vec<(String, Cell)>,
}

impl Measurement {
    pub fn get(&self, column: &str) -> &Cell {
        // later columns with the same name win, as in a dictionary
        self.cells.iter().rev().find(|(k, _)| k == column).map(|(_, c)| c).unwrap_or(&Cell::Empty)
    }
}

#[derive(Clone, Debug)]
pub struct ExcelFile {
    pub file_name: String,
    pub sheet: String,
    pub strength_column: String,
    /// column names in file order, without the strength column
    pub header: Vec<String>,
    pub measurements: Vec<Measurement>,
    /// SHA-256 of the file content (to refuse adding the same file twice)
    pub hash: String,
}

pub fn read_excel(bytes: &[u8], file_name: &str) -> Result<ExcelFile, DataError> {
    let mut wb: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|_| DataError::new("kein_excel"))?;
    let names = wb.sheet_names().to_vec();
    let sheet = SHEET_NAMES.iter().find(|s| names.iter().any(|n| n == *s))
        .ok_or_else(|| DataError::new("blatt_fehlt").with("blaetter", json!(names)))?
        .to_string();
    let range = wb.worksheet_range(&sheet).map_err(|_| DataError::new("kein_excel"))?;
    // absolute grid from A1, as Excel shows it
    let (r0, c0) = range.start().map(|(r, c)| (r as usize, c as usize)).unwrap_or((0, 0));
    let mut rows: Vec<Vec<Cell>> = vec![Vec::new(); r0];
    for r in range.rows() {
        let mut row = vec![Cell::Empty; c0];
        row.extend(r.iter().map(Cell::from));
        rows.push(row);
    }
    let header: Vec<String> = rows.first().map(|h| h.iter()
        .map(|c| cell_text(c).map(|s| s.trim().to_string()).unwrap_or_default()).collect())
        .unwrap_or_default();
    let idx_value = header.iter().position(|k| {
        let k = k.to_lowercase();
        STRENGTH_COLUMN_CONTAINS.iter().any(|t| k.contains(t))
    }).ok_or_else(|| DataError::new("spalte_fehlt"))?;

    let mut measurements = Vec::new();
    for row in rows.iter().skip(1) {
        let first = row.first().unwrap_or(&Cell::Empty);
        if row.is_empty() || *first == Cell::Empty || ["hide", "ausblenden"].contains(&normalize(first).as_str()) {
            continue;
        }
        let value = row.get(idx_value).and_then(to_number);
        let Some(sigma) = value else { continue };
        if sigma <= 0.0 {
            continue;
        }
        let cells = header.iter().enumerate()
            .filter(|(i, k)| !k.is_empty() && *i != idx_value && *i < row.len())
            .map(|(i, k)| (k.clone(), row[i].clone())).collect();
        measurements.push(Measurement { sigma, cells });
    }
    if measurements.is_empty() {
        return Err(DataError::new("keine_werte"));
    }
    Ok(ExcelFile {
        file_name: file_name.into(),
        sheet,
        strength_column: header[idx_value].clone(),
        header: header.iter().enumerate().filter(|(i, k)| !k.is_empty() && *i != idx_value)
            .map(|(_, k)| k.clone()).collect(),
        measurements,
        hash: String::new(),
    })
}
