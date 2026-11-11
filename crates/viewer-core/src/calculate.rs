//! The main calculation for the plots: points, fitted lines, confidence bands
//! and result values of every series, plus common axes.

use crate::area::ring_text;
use crate::data::{series_values, Data};
use crate::results::{evaluate, labels, read_settings, result_values, round_to, series_name, setting_keys};
use glass_stats::dist::paper_y;
use glass_stats::{Distribution, Fit};
use serde_json::{json, Map, Value};

/// Fixed probability axis in percent (the same for every evaluation, so that
/// slopes stay comparable between diagrams).
pub const P_LOW: f64 = 0.2;
pub const P_HIGH: f64 = 99.8;
/// Probability labels by importance; less important ones are dropped first
/// when there is not enough room.
pub const P_TICKS: [f64; 18] = [0.50, 0.05, 0.95, 0.01, 0.99, 0.10, 0.90, 0.20, 0.80, 0.63, 0.31, 0.02, 0.98,
                                0.005, 0.995, 0.002, 0.001, 0.999];
pub const SIGMA_TICKS: [f64; 16] = [5., 10., 15., 20., 30., 40., 50., 60., 80., 100., 120., 150., 200., 250., 300., 400.];
pub const GRID_POINTS: usize = 160;

const DISTS: [(Distribution, &str); 2] = [(Distribution::Weibull, "weibull"), (Distribution::Lognormal, "lognormal")];

fn opt(v: f64) -> Value { if v.is_finite() { json!(v) } else { Value::Null } }

/// Probability labels that lie in the visible range and do not overlap
/// (minimum distance as a share of the axis span), sorted from bottom to top.
pub fn p_ticks(dist: Distribution, y_min: f64, y_max: f64, min_distance: f64) -> Vec<f64> {
    let span = y_max - y_min;
    let mut chosen: Vec<(f64, f64)> = Vec::new();
    for &p in &P_TICKS {
        let y = paper_y(dist, p);
        if !(y_min <= y && y <= y_max) {
            continue;
        }
        if chosen.iter().all(|(_, other)| (y - other).abs() >= min_distance * span) {
            chosen.push((p, y));
        }
    }
    chosen.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    chosen.into_iter().map(|(p, _)| p).collect()
}

/// Percentage label with a decimal comma (one decimal below 1 % and above 99 %).
fn p_label(p: f64) -> String {
    let d = if p < 0.01 || p > 0.99 { 1 } else { 0 };
    format!("{:.*}", d, 100.0 * p).replace('.', ",")
}

fn lines(f: &Fit, grid: &[f64], y_lim: (f64, f64)) -> Value {
    // limit to just outside the axis range instead of cutting off, otherwise
    // the area between the two confidence bounds tears open
    let clip = |v: f64| if v.is_finite() { json!(round_to(v.clamp(y_lim.0 - 1.0, y_lim.1 + 1.0), 5)) } else { Value::Null };
    json!({
        "gerade": grid.iter().map(|&x| clip(f.line_y(x))).collect::<Vec<_>>(),
        "band_unten": grid.iter().map(|&x| clip(f.band_y(x, true))).collect::<Vec<_>>(),
        "band_oben": grid.iter().map(|&x| clip(f.band_y(x, false))).collect::<Vec<_>>(),
    })
}

struct Done {
    out: Map<String, Value>,
    fits: Vec<Fit>,
    originals: Option<Vec<Fit>>,
}

pub fn calculate(data: &Data, req: &Value) -> Value {
    let (settings, target) = read_settings(req);
    let hover: Vec<usize> = req.get("hover_spalten").and_then(|v| v.as_array()).map(|a| a.iter()
        .filter_map(|c| c.as_str().and_then(|c| data.column_index(c))).collect()).unwrap_or_default();
    let mut results: Vec<Result<Done, Value>> = Vec::new();
    let mut hints: Vec<Value> = Vec::new();
    for series in req.get("serien").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let name = series_name(&series);
        let color = series.get("farbe").cloned().unwrap_or(Value::Null);
        let w = series_values(data, &series, target, settings.method, settings.positions);
        let n = w.rows.len();
        let failure = json!({"name": name, "farbe": color, "n": n, "fehler": true});
        if n < 3 {
            hints.push(json!({"name": name, "n": n}));
            results.push(Err(failure));
            continue;
        }
        for h in &w.hints {
            let mut h = h.as_object().cloned().unwrap_or_default();
            h.insert("name".into(), json!(name));
            hints.push(Value::Object(h));
        }
        let fits = match evaluate(&w.sigma, &settings) {
            Ok((a, b)) => vec![a, b],
            Err(text) => { hints.push(json!({"name": name, "text": text})); results.push(Err(failure)); continue; }
        };
        let originals = if w.conversion.is_some() {
            let mut so = w.sigma_orig.clone();
            so.sort_by(|a, b| a.partial_cmp(b).unwrap());
            match evaluate(&so, &settings) {
                Ok((a, b)) => Some(vec![a, b]),
                Err(text) => { hints.push(json!({"name": name, "text": text})); results.push(Err(failure)); continue; }
            }
        } else { None };
        let info: Vec<String> = w.rows.iter().map(|&r| hover.iter()
            .filter(|&&c| !["–", "-"].contains(&data.rows[r].display[c].as_str()))
            .map(|&c| format!("{}: {}", data.columns[c].name, data.rows[r].display[c]))
            .collect::<Vec<_>>().join("<br>")).collect();
        let mut out = Map::new();
        out.insert("name".into(), json!(name));
        out.insert("farbe".into(), color);
        out.insert("n".into(), json!(n));
        out.insert("fehler".into(), json!(false));
        out.insert("sigma".into(), json!(w.sigma.iter().map(|v| round_to(*v, 3)).collect::<Vec<_>>()));
        out.insert("P".into(), json!(fits[0].p.iter().map(|v| round_to(*v, 6)).collect::<Vec<_>>()));
        out.insert("info".into(), json!(info));
        out.insert("umrechnung".into(), w.conversion.clone().unwrap_or(Value::Null));
        if w.conversion.is_some() {
            out.insert("sigma_orig".into(), json!(w.sigma_orig.iter().map(|v| round_to(*v, 3)).collect::<Vec<_>>()));
            out.insert("faktor".into(), json!(w.factors.iter().map(|v| round_to(*v, 5)).collect::<Vec<_>>()));
            out.insert("ring".into(), json!(w.rows.iter().map(|&r| ring_text(data.rows[r].ring.as_deref().unwrap_or("–")))
                .collect::<Vec<_>>()));
        }
        results.push(Ok(Done { out, fits, originals }));
    }

    // common axes; with a conversion also the original values, so that both
    // lines are visible
    let valid: Vec<&Done> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let axes = if valid.is_empty() { Value::Null } else {
        let (p_lo, p_hi) = (P_LOW.min(P_HIGH) / 100.0, P_LOW.max(P_HIGH) / 100.0);
        let y_lim: Vec<(f64, f64)> = DISTS.iter().map(|(d, _)| (paper_y(*d, p_lo), paper_y(*d, p_hi))).collect();
        let mut all_x: Vec<f64> = Vec::new();
        let mut frak_x: Vec<f64> = Vec::new();
        for d in &valid {
            for key in ["sigma", "sigma_orig"] {
                if let Some(v) = d.out.get(key).and_then(|v| v.as_array()) {
                    all_x.extend(v.iter().map(|s| s.as_f64().unwrap().ln()));
                }
            }
            frak_x.extend(d.fits.iter().map(|f| f.frak5_95.ln()));
        }
        let fmin = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        let x_min = fmin(&all_x).min(fmin(&frak_x)) - 0.25;
        let x_max = all_x.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 0.15;
        let step = (x_max - x_min) / (GRID_POINTS - 1) as f64;
        let grid: Vec<f64> = (0..GRID_POINTS).map(|i| if i == GRID_POINTS - 1 { x_max } else { x_min + i as f64 * step }).collect();
        let mut ticks = Map::new();
        let mut limits = Map::new();
        for (k, (d, key)) in DISTS.iter().enumerate() {
            let p = p_ticks(*d, y_lim[k].0, y_lim[k].1, 0.045);
            ticks.insert(key.to_string(), json!({"y": p.iter().map(|&q| paper_y(*d, q)).collect::<Vec<_>>(),
                                                 "text": p.iter().map(|&q| p_label(q)).collect::<Vec<_>>()}));
            limits.insert(key.to_string(), json!([y_lim[k].0, y_lim[k].1]));
        }
        for r in results.iter_mut() {
            let Ok(d) = r else { continue };
            for (k, (dist, key)) in DISTS.iter().enumerate() {
                let f = &d.fits[k];
                let mut e = lines(f, &grid, y_lim[k]).as_object().cloned().unwrap();
                e.insert("y".into(), json!(f.y.iter().map(|v| round_to(*v, 5)).collect::<Vec<_>>()));
                e.insert("kennwerte".into(), result_values(f));
                e.insert("y5".into(), json!(paper_y(*dist, 0.05)));
                if let Some(o) = &d.originals {
                    e.insert("original".into(), json!({"gerade": lines(&o[k], &grid, y_lim[k])["gerade"].clone(),
                                                       "kennwerte": result_values(&o[k])}));
                }
                d.out.insert(key.to_string(), Value::Object(e));
            }
        }
        let (smin, smax) = (x_min.exp(), x_max.exp());
        json!({
            "sigma_gitter": grid.iter().map(|x| round_to(x.exp(), 4)).collect::<Vec<_>>(),
            "sigma_min": smin, "sigma_max": smax,
            "sigma_ticks": SIGMA_TICKS.iter().filter(|&&s| smin <= s && s <= smax).map(|&s| json!(s as i64)).collect::<Vec<_>>(),
            "y_grenzen": limits, "p_ticks": ticks,
        })
    };
    let lab = labels();
    let (m, p, s, l) = setting_keys(&settings);
    json!({
        "serien": results.into_iter().map(|r| match r { Ok(d) => Value::Object(d.out), Err(f) => f }).collect::<Vec<_>>(),
        "achsen": axes, "hinweise": hints,
        "einstellungen": {"methode": lab["methoden"][m], "plotpositionen": lab["plotpositionen"][p],
                          "konfidenz": lab["seiten"][s], "lognormal": lab["lognormal_methoden"][l], "zielflaeche_mm2": opt(target)},
    })
}
