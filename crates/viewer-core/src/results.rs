//! Settings from a request, both fits of a sample, and their result values.

use glass_stats::{fit_lognormal, fit_weibull, Fit, LognormalMethod, PlottingPositions, Settings, Sides,
                  WeibullMethod};
use serde_json::{json, Map, Value};

/// Method, plotting positions, confidence, log-normal estimation and target
/// area from a request; anything unknown falls back to the default.
pub fn read_settings(req: &Value) -> (Settings, f64) {
    let s = |k: &str| req.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let settings = Settings {
        method: match s("methode") {
            "m2_wls" => WeibullMethod::WeightedLeastSquares,
            "m3_glue" => WeibullMethod::Glue,
            _ => WeibullMethod::LeastSquares,
        },
        positions: match s("plotpositionen") {
            "mean" => PlottingPositions::Mean,
            "median" => PlottingPositions::Median,
            _ => PlottingPositions::Hazen,
        },
        sides: if s("konfidenz_seiten") == "einseitig" { Sides::One } else { Sides::Two },
        lognormal: if s("lognormal_methode") == "regression" { LognormalMethod::Regression } else { LognormalMethod::Moments },
    };
    let target = match req.get("zielflaeche_mm2") {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(t)) => t.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    };
    (settings, target)
}

/// Weibull and log-normal fit of a sorted sample.
pub fn evaluate(sorted: &[f64], settings: &Settings) -> Result<(Fit, Fit), String> {
    Ok((fit_weibull(sorted, settings)?, fit_lognormal(sorted, settings)))
}

fn num(v: f64) -> Value {
    if v.is_finite() { json!(v) } else { Value::Null }
}

/// Result values of one fit, as the page's tables expect them.
pub fn result_values(fit: &Fit) -> Value {
    let (a2, d, p) = fit.goodness_of_fit();
    let mut m = Map::new();
    m.insert("n".into(), json!(fit.n));
    match fit.dist {
        glass_stats::Distribution::Weibull => {
            m.insert("lambda".into(), num(fit.shape));
            m.insert("beta".into(), num(fit.scale));
        }
        glass_stats::Distribution::Lognormal => {
            m.insert("mu".into(), num(fit.mu));
            m.insert("sd".into(), num(fit.sd));
            m.insert("var".into(), num(fit.sd * fit.sd));
        }
    }
    for (k, v) in [("R2", fit.r2), ("min", fit.min), ("max", fit.max), ("mean", fit.mean),
                   ("frak5", fit.frak5), ("frak5_95", fit.frak5_95), ("frak5_ob", fit.frak5_ob),
                   ("std", fit.std), ("std_fit", fit.std_fitted()), ("A2", a2), ("D", d), ("p_AD", p)] {
        m.insert(k.into(), num(v));
    }
    Value::Object(m)
}

/// Round like Python's round(x, digits) (correctly rounded decimal).
pub fn round_to(v: f64, digits: usize) -> f64 {
    format!("{:.*}", digits, v).parse().unwrap_or(v)
}

/// Name of a series: "Serie" if none is given; a given name is trimmed (so a
/// name of only spaces becomes empty, as before).
pub fn series_name(series: &Value) -> String {
    match series.get("name") {
        Some(Value::String(s)) if !s.is_empty() => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => "Serie".into(),
    }
}

/// Labels of the select lists (German; the page has the English ones).
pub fn labels() -> Value {
    json!({
        "methoden": {"m1_ls": "Methode 1 – Least-Squares-Regression",
                     "m2_wls": "Methode 2 – gewichtete Least-Squares-Regression (Faucher & Tyson)",
                     "m3_glue": "Methode 3 – Good Linear Unbiased Estimator (EN 12603)"},
        "plotpositionen": {"hazen": "Hazen  P = (i − 0,5)/n", "mean": "Mean Rank  P = i/(n + 1)",
                           "median": "Median Rank  P = (i − 0,3)/(n + 0,4)"},
        "seiten": {"einseitig": "einseitig 95 %", "zweiseitig": "zweiseitig 95 %"},
        "lognormal_methoden": {"moment": "Momentenschätzung",
                               "regression": "Regression im Netz (TU Delft open-GLASSlab)"},
    })
}

/// Keys of the settings (for the labels in the response).
pub fn setting_keys(s: &Settings) -> (&'static str, &'static str, &'static str, &'static str) {
    (match s.method { WeibullMethod::LeastSquares => "m1_ls", WeibullMethod::WeightedLeastSquares => "m2_wls", WeibullMethod::Glue => "m3_glue" },
     match s.positions { PlottingPositions::Hazen => "hazen", PlottingPositions::Mean => "mean", PlottingPositions::Median => "median" },
     match s.sides { Sides::One => "einseitig", Sides::Two => "zweiseitig" },
     match s.lognormal { LognormalMethod::Moments => "moment", LognormalMethod::Regression => "regression" })
}
