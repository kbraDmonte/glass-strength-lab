//! Phase 5: file handling (replace / add / duplicate / remove) and the
//! start-up data.

use serde_json::{json, Value};
use viewer_core::state::Viewer;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");

fn bytes(n: &str) -> Vec<u8> { std::fs::read(format!("{DIR}/data/{n}")).unwrap() }

#[test]
fn files_and_start() {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/golden/phase5.json")).unwrap()).unwrap();
    let mut v = Viewer::new();
    let empty = v.start_response("example.xlsx", true, json!({"A": {"serien": []}}));
    assert_eq!(empty["dateien"], json!([]));
    assert_eq!(empty["zeilen"], json!([]));
    assert_eq!(empty["ansichten"], json!({"A": {"serien": []}}));
    for k in ["palette", "max_upload_mb"] {
        assert_eq!(empty[k], golden["static"][k], "{k}");
    }
    v.upload(&bytes("synth_a.xlsx"), "synth_a.xlsx", "ersetzen").unwrap();
    let e = v.upload(&bytes("synth_a.xlsx"), "renamed.xlsx", "hinzufuegen").unwrap_err();
    assert_eq!(e.to_json(), json!({"fehler_code": "doppelt", "details": {"datei": "renamed.xlsx"}}));
    v.upload(&bytes("synth_b.xlsx"), "synth_b.xlsx", "hinzufuegen").unwrap();
    assert_eq!(v.file_names(), ["synth_a.xlsx", "synth_b.xlsx"]);
    let r = v.start_response("x", false, json!(null));
    assert!(r["spalten"].as_array().unwrap().iter().any(|c| c["name"] == "Excel-Datei"));
    // same name, new content: replaces the old version
    v.remove("synth_b.xlsx");
    assert_eq!(v.file_names(), ["synth_a.xlsx"]);
    v.remove("synth_a.xlsx"); // the last file stays
    assert_eq!(v.file_names(), ["synth_a.xlsx"]);
    let n_a = v.data().rows.len();
    // same name, new content: the newer version replaces the old one
    v.upload(&bytes("synth_b.xlsx"), "synth_a.xlsx", "hinzufuegen").unwrap();
    assert_eq!(v.file_names(), ["synth_a.xlsx"]);
    assert_ne!(v.data().rows.len(), n_a);
    v.upload(&bytes("synth_a.xlsx"), "synth_a.xlsx", "ersetzen").unwrap();
    let e = v.upload(&bytes("bad_not_excel.xlsx"), "bad.xlsx", "ersetzen").unwrap_err();
    assert_eq!(e.code, "kein_excel");
    assert_eq!(v.file_names(), ["synth_a.xlsx"]);
    // calculation via the router equals the direct call
    let req = &golden["cases"][0]["request"];
    v.upload(&bytes("synth_b.xlsx"), "synth_b.xlsx", "hinzufuegen").unwrap();
    assert_eq!(v.request("/api/berechnen", req), viewer_core::calculate::calculate(v.data(), req));
    assert!(v.request("/api/nope", &json!({}))["fehler"].is_string());
}
