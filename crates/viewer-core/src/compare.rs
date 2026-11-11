//! Comparing series: combining two datasets (AD per series, Pearson
//! correlation of Q-Q pairs, two-sample Anderson–Darling) and the group
//! comparison (Kruskal–Wallis + Dunn with compact letter display).

use crate::area::ring_area;
use crate::data::{series_values, Data, SeriesValues};
use crate::results::{evaluate, read_settings, result_values, round_to, series_name};
use crate::text::natural_cmp;
use glass_stats::hypothesis::{anderson_ksamp, dunn, kruskal, letters, pearson};
use serde_json::{json, Value};
use std::collections::HashSet;

pub const GC_ALPHA: f64 = 0.05;
pub const GC_MAX_GROUPS: usize = 30;

/// Test areas (mm², rounded to 0.1) of a series' measurements after a
/// possible conversion; None for measurements without a readable ring size.
fn areas(data: &Data, w: &SeriesValues, target: f64) -> HashSet<Option<i64>> {
    let key = |a: f64| Some((a * 10.0).round() as i64);
    if w.conversion.is_some() {
        return [key(target)].into_iter().collect();
    }
    w.rows.iter().map(|&r| data.rows[r].ring.as_deref().and_then(ring_area).and_then(key)).collect()
}

fn area_info(set: &HashSet<Option<i64>>, target: f64) -> Value {
    let known = set.iter().filter(|a| a.is_some()).count();
    json!({"gleich": known <= 1, "unbekannt": set.contains(&None), "umgerechnet": target > 0.0, "ziel_mm2": target})
}

fn with_name(hints: &[Value], name: &str) -> Vec<Value> {
    hints.iter().map(|h| {
        let mut h = h.as_object().cloned().unwrap_or_default();
        h.insert("name".into(), json!(name));
        Value::Object(h)
    }).collect()
}

/// Quantiles of both samples at equal probability of failure: the smaller
/// sample at its Hazen positions, the larger one interpolated on ln σ.
pub fn qq_pairs(a: &[f64], b: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let swap = a.len() > b.len();
    let (small, large) = if swap { (b, a) } else { (a, b) };
    let pos = |n: usize| (1..=n).map(|i| (i as f64 - 0.5) / n as f64).collect::<Vec<f64>>();
    let (ps, pl) = (pos(small.len()), pos(large.len()));
    let ll: Vec<f64> = large.iter().map(|v| v.ln()).collect();
    let q: Vec<f64> = ps.iter().map(|&p| {
        // numpy.interp: clamp outside, linear inside
        if p <= pl[0] { return ll[0].exp(); }
        if p >= pl[pl.len() - 1] { return ll[ll.len() - 1].exp(); }
        let j = pl.partition_point(|&v| v <= p) - 1;
        let t = (p - pl[j]) / (pl[j + 1] - pl[j]);
        (ll[j] + t * (ll[j + 1] - ll[j])).exp()
    }).collect();
    if swap { (q, small.to_vec()) } else { (small.to_vec(), q) }
}

pub fn combine(data: &Data, req: &Value) -> Value {
    let (settings, target) = read_settings(req);
    let mut parts = Vec::new();
    for key in ["serie_a", "serie_b"] {
        let series = req.get(key).cloned().unwrap_or(json!({}));
        let name = series_name(&series);
        let w = series_values(data, &series, target, settings.method, settings.positions);
        if w.rows.len() < 3 {
            return json!({"fehler_code": "komb_zu_wenig", "details": {"name": name, "n": w.rows.len()}});
        }
        parts.push((name, w));
    }
    let mut set = areas(data, &parts[0].1, target);
    set.extend(areas(data, &parts[1].1, target));
    // combined series: each measurement only once (filters can overlap)
    let mut seen = HashSet::new();
    let mut combined: Vec<f64> = Vec::new();
    for (_, w) in &parts {
        for (r, s) in w.rows.iter().zip(&w.sigma) {
            if seen.insert(*r) {
                combined.push(*s);
            }
        }
    }
    combined.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut series_out = Vec::new();
    for (name, sigma) in [(Some(parts[0].0.clone()), &parts[0].1.sigma), (Some(parts[1].0.clone()), &parts[1].1.sigma),
                          (None, &combined)] {
        match evaluate(sigma, &settings) {
            Ok((w, l)) => series_out.push(json!({"name": name, "n": sigma.len(),
                                                  "weibull": result_values(&w), "lognormal": result_values(&l)})),
            Err(text) => return json!({"fehler_code": "komb_rechnung", "details": {"name": "", "text": text}}),
        }
    }
    let (qa, qb) = qq_pairs(&parts[0].1.sigma, &parts[1].1.sigma);
    let la: Vec<f64> = qa.iter().map(|v| v.ln()).collect();
    let lb: Vec<f64> = qb.iter().map(|v| v.ln()).collect();
    let (r, p) = pearson(&la, &lb);
    let (a2, p_ad) = anderson_ksamp(&[parts[0].1.sigma.clone(), parts[1].1.sigma.clone()]);
    let mut hints = with_name(&parts[0].1.hints, &parts[0].0);
    hints.extend(with_name(&parts[1].1.hints, &parts[1].0));
    json!({
        "serien": series_out,
        "pearson": {"r": r, "p": p, "paare": qa.len()},
        "qq": {"a": qa.iter().map(|v| round_to(*v, 3)).collect::<Vec<_>>(),
               "b": qb.iter().map(|v| round_to(*v, 3)).collect::<Vec<_>>()},
        "ad_zwei": {"A2": a2, "p": p_ad},
        "flaeche": area_info(&set, target),
        "hinweise": hints,
    })
}

struct Group {
    name: String,
    index: Option<Value>,
    key: Option<String>,
    sigma: Vec<f64>,
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len();
    if n % 2 == 1 { s[n / 2] } else { 0.5 * (s[n / 2 - 1] + s[n / 2]) }
}

pub fn group_comparison(data: &Data, req: &Value) -> Value {
    let (settings, target) = read_settings(req);
    let bonferroni = req.get("korrektur").and_then(|v| v.as_str()) == Some("bonferroni");
    let by_column = req.get("modus").and_then(|v| v.as_str()) == Some("spalte");
    let mut hints: Vec<Value> = Vec::new();
    let mut groups: Vec<Group> = Vec::new();
    let mut set: HashSet<Option<i64>> = HashSet::new();
    let mut too_small: Vec<Value> = Vec::new();
    if !by_column {
        let mut seen: HashSet<usize> = HashSet::new();
        let mut duplicates = 0;
        let list = req.get("serien").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for (nr, series) in list.iter().enumerate() {
            let name = series_name(series);
            let w = series_values(data, series, target, settings.method, settings.positions);
            hints.extend(with_name(&w.hints, &name));
            if w.rows.len() < 3 {
                too_small.push(json!({"name": name, "n": w.rows.len()}));
                continue;
            }
            let ids: HashSet<usize> = w.rows.iter().copied().collect();
            duplicates += ids.intersection(&seen).count();
            seen.extend(ids);
            set.extend(areas(data, &w, target));
            groups.push(Group { name, index: Some(series.get("index").cloned().unwrap_or(json!(nr))), key: None,
                                sigma: w.sigma });
        }
        if duplicates > 0 {
            hints.push(json!({"code": "gv_doppelt", "name": "", "n": duplicates}));
        }
    } else {
        let series = req.get("serie").cloned().unwrap_or(json!({}));
        let column = req.get("spalte").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let Some(ci) = data.column_index(&column) else {
            return json!({"fehler_code": "gv_spalte", "details": {"spalte": column}});
        };
        let name = series_name(&series);
        let w = series_values(data, &series, target, settings.method, settings.positions);
        hints.extend(with_name(&w.hints, &name));
        set.extend(areas(data, &w, target));
        let mut parts: Vec<(String, String, Vec<f64>)> = Vec::new();
        for (&r, &s) in w.rows.iter().zip(&w.sigma) {
            let key = &data.rows[r].keys[ci];
            match parts.iter_mut().find(|p| &p.0 == key) {
                Some(p) => p.2.push(s),
                None => parts.push((key.clone(), data.rows[r].display[ci].clone(), vec![s])),
            }
        }
        for (key, display, values) in parts {
            if ["–", "-", ""].contains(&display.as_str()) {
                continue;
            }
            if values.len() < 3 {
                too_small.push(json!({"name": display, "n": values.len()}));
                continue;
            }
            groups.push(Group { name: display, index: None, key: Some(key), sigma: values });
        }
    }
    if groups.len() < 2 {
        return json!({"fehler_code": "gv_zu_wenig", "details": {}, "zu_klein": too_small});
    }
    let truncated = groups.len() > GC_MAX_GROUPS;
    if truncated {
        groups.sort_by(|a, b| b.sigma.len().cmp(&a.sigma.len()));
        groups.truncate(GC_MAX_GROUPS);
    }
    let area = area_info(&set, target);
    if area["gleich"] == false {
        return json!({"fehler_code": "gv_flaeche", "details": {}, "flaeche": area});
    }
    if by_column {
        groups.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    }
    let samples: Vec<Vec<f64>> = groups.iter().map(|g| g.sigma.clone()).collect();
    let (h, p_kw) = kruskal(&samples);
    let (mean_ranks, pairs) = dunn(&samples, bonferroni);
    let mut ranking: Vec<usize> = (0..groups.len()).collect();
    let medians: Vec<f64> = samples.iter().map(|s| median(s)).collect();
    ranking.sort_by(|&a, &b| medians[b].partial_cmp(&medians[a]).unwrap());
    let lett = letters(groups.len(), &pairs, GC_ALPHA, &ranking);
    let out: Vec<Value> = groups.iter().enumerate().map(|(i, g)| {
        let mut o = serde_json::Map::new();
        o.insert("name".into(), json!(g.name));
        if let Some(ix) = &g.index { o.insert("index".into(), ix.clone()); }
        if let Some(k) = &g.key { o.insert("schluessel".into(), json!(k)); }
        o.insert("n".into(), json!(g.sigma.len()));
        o.insert("median".into(), json!(medians[i]));
        o.insert("mittelrang".into(), json!(mean_ranks[i]));
        o.insert("buchstaben".into(), json!(lett[i]));
        Value::Object(o)
    }).collect();
    let n_total: usize = samples.iter().map(|s| s.len()).sum();
    json!({
        "modus": if by_column { "spalte" } else { "serien" },
        "korrektur": if bonferroni { "bonferroni" } else { "holm" },
        "alpha": GC_ALPHA,
        "kw": {"H": h, "df": groups.len() - 1, "p": p_kw, "N": n_total},
        "gruppen": out,
        "paare": pairs.iter().map(|p| json!({"i": p.i, "j": p.j, "z": p.z, "p_roh": p.p_raw, "p": p.p})).collect::<Vec<_>>(),
        "flaeche": area,
        "zu_klein": too_small,
        "gekuerzt": truncated,
        "hinweise": hints,
    })
}
