//! Data table (/api/zeilen) and points against another series' line
//! (/api/punkte_gerade).

use serde_json::json;
use viewer_core::state::Viewer;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");

fn viewer() -> Viewer {
    let mut v = Viewer::new();
    let b = std::fs::read(format!("{DIR}/data/synth_a.xlsx")).unwrap();
    v.upload(&b, "synth_a.xlsx", "ersetzen").unwrap();
    v
}

#[test]
fn table_has_all_columns_and_the_series_rows() {
    let v = viewer();
    let d = v.data();
    let col = &d.columns[0];
    let key = &col.values[0].key;
    let filter = json!({col.name.clone(): [key]});
    let r = v.request("/api/zeilen", &json!({"serien": [{"name": "alle", "filter": {}}, {"name": "x", "filter": filter}]}));
    let cols = r["spalten"].as_array().unwrap();
    assert_eq!(cols.len(), d.header.len() + 1);
    assert_eq!(cols.last().unwrap(), &json!(d.strength_column));
    let all = r["serien"][0]["zeilen"].as_array().unwrap();
    assert_eq!(all.len(), d.rows.len());
    assert!(all.iter().all(|z| z.as_array().unwrap().len() == cols.len()));
    // file order, σ last
    assert_eq!(all[0].as_array().unwrap().last().unwrap().as_f64().unwrap(), d.rows[0].sigma);
    assert_eq!(r["serien"][1]["zeilen"].as_array().unwrap().len(), col.values[0].n);
}

#[test]
fn points_against_own_line_and_shifted_line() {
    let v = viewer();
    let s = json!({"name": "alle", "filter": {}});
    let req = json!({"serie_a": s, "serie_b": s, "methode": "m1_ls", "plotpositionen": "hazen"});
    let r = v.request("/api/punkte_gerade", &req);
    for dist in ["weibull", "lognormal"] {
        let d = &r[dist];
        let (ra, rb) = (d["rms_a"].as_f64().unwrap(), d["rms_b"].as_f64().unwrap());
        assert!((ra - rb).abs() < 1e-12, "{dist}");
        assert!((d["verhaeltnis"].as_f64().unwrap() - 1.0).abs() < 1e-9, "{dist}");
        let p = d["ks"]["p"].as_f64().unwrap();
        assert!((0.0..=1.0).contains(&p), "{dist}");
        let y = d["b"]["y"].as_array().unwrap();
        assert_eq!(y.len(), d["b"]["linie_a"].as_array().unwrap().len());
    }
    // a series against a different one: residuals measured to the other line
    let col = &v.data().columns[0];
    let one = json!({"name": "x", "filter": {col.name.clone(): [col.values[0].key.clone()]}});
    let r = v.request("/api/punkte_gerade", &json!({"serie_a": one, "serie_b": s}));
    let w = &r["weibull"];
    assert!(w["rms_a"].as_f64().unwrap() >= w["rms_b"].as_f64().unwrap() - 1e-12);
    assert_eq!(r["namen"], json!(["x", "alle"]));
    let few = json!({"name": "leer", "filter": {col.name.clone(): ["gibt es nicht"]}});
    assert_eq!(v.request("/api/punkte_gerade", &json!({"serie_a": few, "serie_b": s}))["fehler_code"], "komb_zu_wenig");
}

#[test]
fn xlsx_export_reads_back() {
    use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
    let v = viewer();
    let t = v.request("/api/zeilen", &json!({"serien": [{"name": "alle", "filter": {}}]}));
    let rows = t["serien"][0]["zeilen"].clone();
    let bytes = viewer_core::xlsx::xlsx(&json!({"blaetter": [
        {"name": "G1_1/Pos.1 <Air> & W", "spalten": t["spalten"], "zeilen": rows},
        {"name": "zwei", "spalten": ["a", "b"], "zeilen": [["x", 1.5], ["–", "007"]]}]}));
    let mut wb: Xlsx<_> = open_workbook_from_rs(std::io::Cursor::new(bytes.clone())).unwrap();
    assert_eq!(wb.sheet_names(), ["G1_1Pos.1 <Air> & W", "zwei"]);
    let r = wb.worksheet_range("G1_1Pos.1 <Air> & W").unwrap();
    let d = v.data();
    assert_eq!(r.height(), d.rows.len() + 1);
    assert_eq!(r.get((0, d.header.len())), Some(&Data::String(d.strength_column.clone())));
    // σ as a number, equal to the data
    assert_eq!(r.get((1, d.header.len())), Some(&Data::Float(d.rows[0].sigma)));
    let r2 = wb.worksheet_range("zwei").unwrap();
    assert_eq!(r2.get((1, 1)), Some(&Data::Float(1.5)));
    assert_eq!(r2.get((2, 1)), Some(&Data::String("007".into())));   // codes stay text
    assert!(matches!(r2.get((2, 0)), None | Some(Data::Empty)));     // "–" = empty
    // and the app itself can load the exported table again
    let mut v2 = viewer_core::state::Viewer::new();
    let one = viewer_core::xlsx::xlsx(&json!({"blaetter": [{"name": "Dataset", "spalten": t["spalten"], "zeilen": t["serien"][0]["zeilen"]}]}));
    v2.upload(&one, "export.xlsx", "ersetzen").unwrap();
    assert_eq!(v2.data().rows.len(), d.rows.len());
    let _ = bytes;
}

#[test]
fn points_vs_line_bands_contain_the_lines() {
    let v = viewer();
    let s = json!({"name": "alle", "filter": {}});
    let r = v.request("/api/punkte_gerade", &json!({"serie_a": s, "serie_b": s}));
    for dist in ["weibull", "lognormal"] {
        let d = &r[dist];
        let b = &d["band"];
        let sig = b["sigma"].as_array().unwrap();
        assert_eq!(sig.len(), 81);
        for (i, s) in sig.iter().enumerate() {
            let x = s.as_f64().unwrap().ln();
            let line = d["a"]["steigung"].as_f64().unwrap() * x + d["a"]["achse"].as_f64().unwrap();
            let (lo, hi) = (b["a_oben"][i].as_f64().unwrap(), b["a_unten"][i].as_f64().unwrap());
            assert!(lo <= line + 1e-5 && line <= hi + 1e-5, "{dist} {i}: {lo} {line} {hi}");
        }
    }
}
