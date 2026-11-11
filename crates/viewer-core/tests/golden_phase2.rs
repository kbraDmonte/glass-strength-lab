//! Phase 2: reading Excel files, filter columns, series selection and the
//! test-area conversion against expected values (tests/golden/phase2.json).

use glass_stats::{PlottingPositions, WeibullMethod};
use serde_json::Value;
use viewer_core::{build_data, data_response, read_excel, series_values, ExcelFile};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");

fn read(name: &str) -> Result<ExcelFile, viewer_core::DataError> {
    let bytes = std::fs::read(format!("{DIR}/data/{name}")).unwrap();
    read_excel(&bytes, name)
}

/// Structural comparison; numbers within a relative tolerance.
fn same(a: &Value, b: &Value, path: &str, errors: &mut Vec<String>) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if (x - y).abs() > 1e-9 + 1e-9 * y.abs() {
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
fn phase2_matches_expected_values() {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/golden/phase2.json")).unwrap()).unwrap();
    let mut errors = Vec::new();

    for (name, expected) in golden["errors"].as_object().unwrap() {
        match read(name) {
            Ok(_) => errors.push(format!("{name}: read without error")),
            Err(e) => {
                let got = serde_json::json!({"error": e.code, "details": e.details});
                same(&got, expected, name, &mut errors);
            }
        }
    }

    for (set, g) in golden["datasets"].as_object().unwrap() {
        let files: Vec<ExcelFile> = g["files"].as_array().unwrap().iter()
            .map(|f| read(f.as_str().unwrap()).unwrap()).collect();
        let data = build_data(&files);
        same(&data_response(&data), &g["response"], &format!("{set}.response"), &mut errors);

        for (i, c) in g["cases"].as_array().unwrap().iter().enumerate() {
            let method = match c["method"].as_str().unwrap() {
                "m1_ls" => WeibullMethod::LeastSquares,
                "m2_wls" => WeibullMethod::WeightedLeastSquares,
                _ => WeibullMethod::Glue,
            };
            let positions = match c["positions"].as_str().unwrap() {
                "hazen" => PlottingPositions::Hazen,
                "mean" => PlottingPositions::Mean,
                _ => PlottingPositions::Median,
            };
            let w = series_values(&data, &c["series"], c["target_mm2"].as_f64().unwrap(), method, positions);
            let got = serde_json::json!({
                "n": w.rows.len(), "sigma": w.sigma, "sigma_orig": w.sigma_orig, "factors": w.factors,
                "row_keys": w.rows.iter().map(|&r| {
                    data.columns.iter().zip(&data.rows[r].keys)
                        .map(|(col, k)| (col.name.clone(), Value::String(k.clone())))
                        .collect::<serde_json::Map<_, _>>()
                }).collect::<Vec<_>>(),
                "conversion": w.conversion, "hints": w.hints,
            });
            let mut exp = c.clone();
            let exp = exp.as_object_mut().unwrap();
            for k in ["series", "target_mm2", "method", "positions"] { exp.remove(k); }
            let id = format!("{set}.case{i} ({} target {} {})", c["series"]["name"], c["target_mm2"], c["method"]);
            same(&got, &Value::Object(exp.clone()), &id, &mut errors);
        }
    }
    let shown: Vec<&String> = errors.iter().take(25).collect();
    assert!(errors.is_empty(), "{} deviations:\n{}", errors.len(),
            shown.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"));
}
