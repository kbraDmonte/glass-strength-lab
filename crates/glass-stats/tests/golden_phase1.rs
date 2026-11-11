//! Phase 1: fits, bands, fractiles and goodness of fit against expected values
//! from the previous implementation (tests/golden/phase1.json, synthetic data).

use glass_stats::{fit_lognormal, fit_weibull, Fit, LognormalMethod, PlottingPositions, Settings,
                  Sides, WeibullMethod};
use serde_json::Value;
use std::collections::BTreeMap;

const REL: f64 = 1e-7;
const ABS: f64 = 1e-9;

fn settings(c: &Value) -> Settings {
    Settings {
        method: match c["method"].as_str().unwrap() {
            "m1_ls" => WeibullMethod::LeastSquares,
            "m2_wls" => WeibullMethod::WeightedLeastSquares,
            _ => WeibullMethod::Glue,
        },
        positions: match c["positions"].as_str().unwrap() {
            "hazen" => PlottingPositions::Hazen,
            "mean" => PlottingPositions::Mean,
            _ => PlottingPositions::Median,
        },
        sides: if c["sides"] == "einseitig" { Sides::One } else { Sides::Two },
        lognormal: if c["lognormal_method"] == "regression" { LognormalMethod::Regression } else { LognormalMethod::Moments },
    }
}

fn values(fit: &Fit, weibull: bool) -> BTreeMap<&'static str, f64> {
    let (a2, d, p) = fit.goodness_of_fit();
    let mut m = BTreeMap::new();
    if weibull {
        m.insert("lambda", fit.shape);
        m.insert("beta", fit.scale);
    } else {
        m.insert("mu", fit.mu);
        m.insert("sd", fit.sd);
        m.insert("var", fit.sd * fit.sd);
    }
    for (k, v) in [("R2", fit.r2), ("min", fit.min), ("max", fit.max), ("mean", fit.mean),
                   ("std", fit.std), ("frak5", fit.frak5), ("frak5_95", fit.frak5_95),
                   ("frak5_ob", fit.frak5_ob), ("slope", fit.slope), ("intercept", fit.intercept),
                   ("x0", fit.x0), ("A2", a2), ("D", d), ("p_AD", p), ("std_fit", fit.std_fitted())] {
        m.insert(k, v);
    }
    m
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= ABS + REL * b.abs()
}

#[test]
fn phase1_matches_expected_values() {
    let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/golden/phase1.json");
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(pfad).unwrap()).unwrap();
    let mut fehler: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut geprueft = 0;
    let mut notiere = |schluessel: String, beispiel: String| {
        let e = fehler.entry(schluessel).or_insert((0, beispiel));
        e.0 += 1;
    };
    for c in golden["cases"].as_array().unwrap() {
        let mut s: Vec<f64> = c["values"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let st = settings(c);
        let id = format!("{} n={} {} {} {} {}", c["sample"], c["n"], c["method"], c["positions"], c["sides"], c["lognormal_method"]);
        let w = fit_weibull(&s, &st).expect("weibull fit");
        let l = fit_lognormal(&s, &st);
        for (name, fit, weibull) in [("weibull", &w, true), ("lognormal", &l, false)] {
            let e = &c[name];
            for (k, v) in values(fit, weibull) {
                let erwartet = &e[k];
                if erwartet.is_null() { continue; }
                let x = erwartet.as_f64().unwrap();
                geprueft += 1;
                if !close(v, x) {
                    notiere(format!("{name}.{k} [{}]", c["method"].as_str().unwrap()),
                            format!("{id}: got {v}, expected {x}"));
                }
            }
            for (i, q) in e["P"].as_array().unwrap().iter().enumerate() {
                geprueft += 1;
                if !close(fit.p[i], q.as_f64().unwrap()) { notiere(format!("{name}.P"), id.clone()); }
            }
            let xs: Vec<f64> = e["band_x"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
            for (seite, lower) in [("band_lower", true), ("band_upper", false)] {
                if e[seite].is_null() { continue; }
                for (i, b) in e[seite].as_array().unwrap().iter().enumerate() {
                    if b.is_null() { continue; }
                    geprueft += 1;
                    let v = fit.band_y(xs[i], lower);
                    if !close(v, b.as_f64().unwrap()) {
                        notiere(format!("{name}.{seite} [{}]", c["method"].as_str().unwrap()),
                                format!("{id} x={}: got {v}, expected {}", xs[i], b));
                    }
                }
            }
        }
    }
    let mut bericht = String::new();
    for (k, (n, bsp)) in &fehler {
        bericht += &format!("\n  {k}: {n} deviations, e.g. {bsp}");
    }
    println!("checked {geprueft} values");
    assert!(fehler.is_empty(), "deviations from expected values:{bericht}");
}
