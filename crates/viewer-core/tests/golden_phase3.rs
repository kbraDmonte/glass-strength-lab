//! Phase 3: combining two datasets and the group comparison (Kruskal–Wallis +
//! Dunn) against expected values (tests/golden/phase3.json).

use serde_json::Value;
use viewer_core::compare::{combine, group_comparison};
use viewer_core::{build_data, read_excel, ExcelFile};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");

fn same(a: &Value, b: &Value, path: &str, errors: &mut Vec<String>) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if (x - y).abs() > 1e-12 + 1e-7 * y.abs() {
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
fn phase3_matches_expected_values() {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/golden/phase3.json")).unwrap()).unwrap();
    let mut errors = Vec::new();
    let mut count = 0;
    for (set, g) in golden.as_object().unwrap() {
        let files: Vec<ExcelFile> = g["files"].as_array().unwrap().iter().map(|f| {
            let name = f.as_str().unwrap();
            read_excel(&std::fs::read(format!("{DIR}/data/{name}")).unwrap(), name).unwrap()
        }).collect();
        let data = build_data(&files);
        for (i, c) in g["cases"].as_array().unwrap().iter().enumerate() {
            let got = match c["endpoint"].as_str().unwrap() {
                "kombinieren" => combine(&data, &c["request"]),
                _ => group_comparison(&data, &c["request"]),
            };
            count += 1;
            same(&got, &c["response"], &format!("{set}.case{i}.{}", c["endpoint"].as_str().unwrap()), &mut errors);
        }
    }
    assert!(count > 100, "too few cases");
    let shown: Vec<&String> = errors.iter().take(25).collect();
    assert!(errors.is_empty(), "{} deviations:\n{}", errors.len(),
            shown.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"));
}
