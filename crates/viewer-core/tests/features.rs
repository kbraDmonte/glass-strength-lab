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
