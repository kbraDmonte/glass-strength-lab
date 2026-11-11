//! Phase 4: validation suite against expected values (tests/golden/phase4.json).
//! Deterministic parts (leave-one-out for n ≤ 500, leave-one-group-out, all
//! counts and settings) must match exactly; the randomised checks use another
//! random generator than the previous implementation and are compared
//! statistically (same conclusions within sampling error).

use serde_json::Value;
use viewer_core::validation::validate;
use viewer_core::{build_data, read_excel};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests");

fn same(a: &Value, b: &Value, path: &str, errors: &mut Vec<String>) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            // KS p-values far in the tail use an approximation: absolute tolerance
            let tol = if path.contains(".p") { 1e-6 } else { 1e-12 };
            if (x - y).abs() > tol + 1e-7 * y.abs() {
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
        }
        _ => if a != b { errors.push(format!("{path}: got {a}, expected {b}")) },
    }
}

fn f(v: &Value) -> f64 { v.as_f64().unwrap() }

#[test]
fn phase4_validation() {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/golden/phase4.json")).unwrap()).unwrap();
    let files: Vec<_> = golden["files"].as_array().unwrap().iter().map(|n| {
        let n = n.as_str().unwrap();
        read_excel(&std::fs::read(format!("{DIR}/data/{n}")).unwrap(), n).unwrap()
    }).collect();
    let data = build_data(&files);
    let mut errors = Vec::new();
    let mut table = vec![format!("{:<12} {:<9} {:>9} {:>9}   {:>8} {:>8}   {:>8} {:>8}   {:>7} {:>7}",
                                 "series", "dist", "ll old", "ll new", "<5% old", "<5% new", "rej old", "rej new", "gof old", "gof new")];
    for (i, c) in golden["cases"].as_array().unwrap().iter().enumerate() {
        let t0 = std::time::Instant::now();
        let got = validate(&data, &c["request"]);
        let ms = t0.elapsed().as_millis();
        let exp = &c["response"];
        let id = format!("case{i} ({})", c["request"]["serie"]["name"]);
        if exp.get("fehler_code").is_some() {
            same(&got, exp, &id, &mut errors);
            continue;
        }
        for k in ["name", "n", "k", "gruppen_spalte", "umrechnung", "hinweise", "wdh", "split_n", "split_anteil",
                  "boot_n", "gof_n", "loo_max"] {
            same(&got[k], &exp[k], &format!("{id}.{k}"), &mut errors);
        }
        for dist in ["weibull", "lognormal"] {
            let (g, e) = (&got["arten"][dist], &exp["arten"][dist]);
            let p = format!("{id}.{dist}");
            // deterministic
            same(&g["voll"], &e["voll"], &format!("{p}.voll"), &mut errors);
            if !e["loo"]["auswahl"].as_bool().unwrap() {
                same(&g["loo"], &e["loo"], &format!("{p}.loo"), &mut errors);
            }
            if e.get("gruppen").is_some() {
                same(&g["gruppen"], &e["gruppen"], &format!("{p}.gruppen"), &mut errors);
            }
            for k in ["n_lern", "n_test"] {
                same(&g["split"][k], &e["split"][k], &format!("{p}.split.{k}"), &mut errors);
            }
            same(&g["kfold"]["unter5"]["n"], &e["kfold"]["unter5"]["n"], &format!("{p}.kfold.n"), &mut errors);
            same(&g["bootstrap"]["frak5_95"]["voll"], &e["bootstrap"]["frak5_95"]["voll"], &format!("{p}.boot.voll"), &mut errors);
            // statistical
            let n = f(&exp["n"]);
            let mut check = |name: &str, a: f64, b: f64, tol: f64| {
                if (a - b).abs() > tol {
                    errors.push(format!("{p}.{name}: new {a:.4}, old {b:.4} (tolerance {tol:.4})"));
                }
            };
            check("kfold.ll", f(&g["kfold"]["ll"]), f(&e["kfold"]["ll"]), 0.03);
            check("kfold.unter5", f(&g["kfold"]["unter5"]["anteil"]), f(&e["kfold"]["unter5"]["anteil"]), 3.0 * (0.05 * 0.95 / n).sqrt());
            check("split.anteil5_mittel", f(&g["split"]["anteil5_mittel"]), f(&e["split"]["anteil5_mittel"]), 3.0 * (0.05 * 0.95 / n).sqrt());
            // the 200 splits share data, so the rate varies more than a binomial
            // share: seed-to-seed SD measured at 0.029 (Lab A, 50/50) -> ~4 SD
            check("split.abgelehnt", f(&g["split"]["abgelehnt"]), f(&e["split"]["abgelehnt"]), 0.12);
            for q in ["5", "50", "95"] {
                let (a, b) = (f(&g["bootstrap"]["frak5"]["q"][q]), f(&e["bootstrap"]["frak5"]["q"][q]));
                check(&format!("boot.frak5.q{q}"), a / b, 1.0, 0.03);
            }
            let (ga, gb) = (f(&g["gof"]["p"]), f(&e["gof"]["p"]));
            check("gof.p", ga, gb, 0.1_f64.max(0.5 * gb));
            if (ga < 0.05) != (gb < 0.05) && (ga - gb).abs() > 0.02 {
                errors.push(format!("{p}.gof.p: verdict differs (new {ga}, old {gb})"));
            }
            table.push(format!("{:<12} {:<9} {:>9.4} {:>9.4}   {:>8.4} {:>8.4}   {:>8.3} {:>8.3}   {:>7.3} {:>7.3}  ({ms} ms)",
                c["request"]["serie"]["name"].as_str().unwrap(), dist, f(&e["kfold"]["ll"]), f(&g["kfold"]["ll"]),
                f(&e["kfold"]["unter5"]["anteil"]), f(&g["kfold"]["unter5"]["anteil"]),
                f(&e["split"]["abgelehnt"]), f(&g["split"]["abgelehnt"]), gb, ga));
        }
    }
    println!("{}", table.join("\n"));
    let shown: Vec<&String> = errors.iter().take(30).collect();
    assert!(errors.is_empty(), "{} deviations:\n{}", errors.len(),
            shown.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"));
}
