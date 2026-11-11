//! WebAssembly wrapper: one `Viewer` object holds the loaded files; every
//! method takes and returns JSON text (the page's protocol is unchanged).

use serde_json::{json, Value};
use viewer_core::state;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Viewer(state::Viewer);

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(json!({}))
}

#[wasm_bindgen]
impl Viewer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Viewer {
        Viewer(state::Viewer::new())
    }

    /// Start-up data (`/api/daten`); `views` = saved views as JSON text.
    pub fn daten(&self, default_name: &str, default_present: bool, views: &str) -> String {
        self.0.start_response(default_name, default_present, parse(views)).to_string()
    }

    /// Upload; returns "" on success, otherwise the error JSON
    /// (`{"fehler_code", "details"}`).
    pub fn hochladen(&mut self, bytes: &[u8], file_name: &str, mode: &str) -> String {
        match self.0.upload(bytes, file_name, mode) {
            Ok(()) => String::new(),
            Err(e) => e.to_json().to_string(),
        }
    }

    pub fn entfernen(&mut self, file_name: &str) {
        self.0.remove(file_name);
    }

    /// Names of the loaded files, as a JSON list.
    pub fn dateien(&self) -> String {
        json!(self.0.file_names()).to_string()
    }

    /// Calculation requests: /api/berechnen, /api/kombinieren,
    /// /api/gruppenvergleich, /api/validieren.
    pub fn anfrage(&self, path: &str, body: &str) -> String {
        self.0.request(path, &parse(body)).to_string()
    }
}

impl Default for Viewer {
    fn default() -> Self {
        Self::new()
    }
}
