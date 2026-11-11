//! Phase 5: main calculation for the plots against expected values
//! (tests/golden/phase5.json).

use serde_json::Value;
use viewer_core::calculate::{calculate, P_HIGH, P_LOW, P_TICKS, SIGMA_TICKS};
use viewer_core::{build_data, read_excel};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");

fn same(a: &Value, b: &Value, path: &str, errors: &mut Vec<String>) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if (x - y).abs() > 1e-9 + 1e-7 * y.abs() {
                errors.push(format!("{path}: got {x}, expected {y}"));
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                errors.push(format!("{path}: length {} vs expected {}", x.len(), y.len()));
                return;
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                same(p, q, &format!("{path}[{i}]"), errors);
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for (k, q) in y {
                match x.get(k) {
                    Some(p) => same(p, q, &format!("{path}.{k}"), errors),
                    None => errors.push(format!("{path}.{k}: missing")),
                }
            }
            for k in x.keys() {
                if !y.contains_key(k) {
                    errors.push(format!("{path}.{k}: unexpected"));
                }
            }
        }
        _ => if a != b { errors.push(format!("{path}: got {a}, expected {b}")) },
    }
}

#[test]
fn phase5_calculate() {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/golden/phase5.json")).unwrap()).unwrap();
    let files: Vec<_> = ["synth_a.xlsx", "synth_b.xlsx"].iter()
        .map(|n| read_excel(&std::fs::read(format!("{DIR}/data/{n}")).unwrap(), n).unwrap()).collect();
    let data = build_data(&files);
    let mut errors = Vec::new();
    let k = &golden["constants"];
    assert_eq!(k["P_UNTEN"].as_f64().unwrap(), P_LOW);
    assert_eq!(k["P_OBEN"].as_f64().unwrap(), P_HIGH);
    assert_eq!(k["P_TICKS"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>(), P_TICKS.to_vec());
    assert_eq!(k["SIGMA_TICKS"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>(), SIGMA_TICKS.to_vec());
    let st = &golden["static"];
    let lab = viewer_core::results::labels();
    for key in ["methoden", "plotpositionen", "seiten", "lognormal_methoden"] {
        same(&lab[key], &st[key], key, &mut errors);
    }
    for (i, c) in golden["cases"].as_array().unwrap().iter().enumerate() {
        let got = calculate(&data, &c["request"]);
        same(&got, &c["response"], &format!("case{i}"), &mut errors);
    }
    let shown: Vec<&String> = errors.iter().take(30).collect();
    assert!(errors.is_empty(), "{} deviations:\n{}", errors.len(),
            shown.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"));
}
