//! The loaded files and the data built from them, plus the request routing
//! of the previous server (same paths, same JSON), so that a thin wrapper
//! (WebAssembly in the browser, or a test) can answer the page.

use crate::data::{build_data, data_response, Data};
use crate::excel::{read_excel, DataError, ExcelFile};
use crate::results::labels;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const PALETTE: [&str; 10] = ["#2a78d6", "#eb6834", "#1baf7a", "#4a3aa7", "#8a6d00", "#c43a7a", "#00868a",
                                 "#a34b00", "#6b6b66", "#d4a20b"];
pub const MAX_UPLOAD_MB: usize = 50;

pub struct Viewer {
    files: Vec<ExcelFile>,
    data: Data,
}

impl Default for Viewer {
    fn default() -> Self { Self::new() }
}

/// Read an Excel file from bytes and add a checksum (detects files uploaded
/// twice, so that no measurement is counted twice).
pub fn load_file(bytes: &[u8], file_name: &str) -> Result<ExcelFile, DataError> {
    if bytes.len() > MAX_UPLOAD_MB * 1024 * 1024 {
        return Err(DataError::new("zu_gross").with("mb", json!(MAX_UPLOAD_MB)));
    }
    let mut f = read_excel(bytes, file_name)?;
    f.hash = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    Ok(f)
}

impl Viewer {
    pub fn new() -> Self {
        Viewer { files: Vec::new(), data: build_data(&[]) }
    }

    pub fn data(&self) -> &Data { &self.data }

    pub fn file_names(&self) -> Vec<String> { self.files.iter().map(|f| f.file_name.clone()).collect() }

    fn set(&mut self, files: Vec<ExcelFile>) {
        self.data = build_data(&files);
        self.files = files;
    }

    /// Start-up data of the page. `default_name`/`default_present` describe
    /// the example file, `views` the saved views (both kept by the caller).
    pub fn start_response(&self, default_name: &str, default_present: bool, views: Value) -> Value {
        let mut r = data_response(&self.data);
        let lab = labels();
        let o = r.as_object_mut().unwrap();
        o.insert("standard".into(), json!(default_name));
        o.insert("standard_vorhanden".into(), json!(default_present));
        o.insert("palette".into(), json!(PALETTE));
        for k in ["methoden", "plotpositionen", "seiten", "lognormal_methoden"] {
            o.insert(k.into(), lab[k].clone());
        }
        o.insert("ansichten".into(), if views.is_object() { views } else { json!({}) });
        o.insert("max_upload_mb".into(), json!(MAX_UPLOAD_MB));
        r
    }

    /// Upload: mode "hinzufuegen" adds the file (a file with the same name is
    /// replaced by the newer version, identical content is refused), anything
    /// else replaces all files.
    pub fn upload(&mut self, bytes: &[u8], file_name: &str, mode: &str) -> Result<(), DataError> {
        let new_file = load_file(bytes, file_name)?;
        let files = if mode == "hinzufuegen" {
            if self.files.iter().any(|f| f.hash == new_file.hash) {
                return Err(DataError::new("doppelt").with("datei", json!(file_name)));
            }
            let mut files: Vec<ExcelFile> = self.files.iter().filter(|f| f.file_name != file_name).cloned().collect();
            files.push(new_file);
            files
        } else {
            vec![new_file]
        };
        self.set(files);
        Ok(())
    }

    /// Remove a file (never the last one).
    pub fn remove(&mut self, file_name: &str) {
        let files: Vec<ExcelFile> = self.files.iter().filter(|f| f.file_name != file_name).cloned().collect();
        if !files.is_empty() && files.len() < self.files.len() {
            self.set(files);
        }
    }

    /// Answer a calculation request (same paths and JSON as the old server).
    pub fn request(&self, path: &str, req: &Value) -> Value {
        // (non-finite numbers become null already in serde_json)
        match path {
            "/api/berechnen" => crate::calculate::calculate(&self.data, req),
            "/api/kombinieren" => crate::compare::combine(&self.data, req),
            "/api/gruppenvergleich" => crate::compare::group_comparison(&self.data, req),
            "/api/validieren" => crate::validation::validate(&self.data, req),
            _ => json!({"fehler": format!("nicht gefunden: {path}")}),
        }
    }
}
