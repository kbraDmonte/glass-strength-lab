//! Validation of the fit: does the fitted distribution also describe
//! measurements that were not used for the fit? Six checks, for Weibull and
//! log-normal side by side, all with the same fitting functions and settings
//! as the main calculation. The random generator has a fixed seed, so the same
//! request gives the same result.

use crate::data::{series_values, Data};
use crate::results::{read_settings, round_to, series_name};
use glass_stats::hypothesis::{binom_test, holm, percentile};
use glass_stats::ks::{ks_pvalue, ks_statistic};
use glass_stats::random::Rng;
use glass_stats::{fit_lognormal, fit_weibull, Distribution, Fit, Settings};
use serde_json::{json, Value};

pub const VALID_SEED: u64 = 20_261_027;
pub const VALID_MIN_N: usize = 10;
pub const VALID_K_REPEATS: usize = 10;
pub const VALID_SPLIT_SHARE: f64 = 0.7;
pub const VALID_SPLIT_SHARES: [f64; 6] = [0.5, 0.6, 0.7, 0.75, 0.8, 0.9];
pub const VALID_SPLIT_N: usize = 200;
pub const VALID_BOOT_N: usize = 1000;
pub const VALID_GOF_N: usize = 500;
pub const VALID_LOO_MAX: usize = 500;
pub const VALID_MAX_GROUPS: usize = 60;

fn sorted(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

fn fit(dist: Distribution, sample: &[f64], s: &Settings) -> Result<Fit, String> {
    let sample = sorted(sample.to_vec());
    match dist {
        Distribution::Weibull => fit_weibull(&sample, s),
        Distribution::Lognormal => Ok(fit_lognormal(&sample, s)),
    }
}

fn log_density(f: &Fit, x: f64) -> f64 {
    match f.dist {
        Distribution::Weibull => {
            let (m, b) = (f.shape, f.scale);
            let z = x / b;
            (m / b).ln() + (m - 1.0) * z.ln() - z.powf(m)
        }
        Distribution::Lognormal => {
            let u = (x.ln() - f.mu) / f.sd;
            -0.5 * u * u - (f.sd * x * (2.0 * std::f64::consts::PI).sqrt()).ln()
        }
    }
}

fn params(f: &Fit) -> [f64; 4] {
    match f.dist {
        Distribution::Weibull => [f.shape, f.scale, f.frak5, f.frak5_95],
        Distribution::Lognormal => [f.mu, f.sd, f.frak5, f.frak5_95],
    }
}

fn round_half_even(v: f64) -> usize {
    let r = v.round();
    let r = if (v - v.trunc()).abs() == 0.5 && (r as i64) % 2 != 0 { r - v.signum() } else { r };
    r.max(0.0) as usize
}

/// Share below the fractile, with p-value against the expected 5 %.
fn share_test(matches: f64, n: usize) -> Value {
    let k = round_half_even(matches);
    json!({"anteil": matches / n as f64, "k": k, "n": n, "p": binom_test(k, n, 0.05)})
}

fn ks_p(sample: &[f64], f: &Fit) -> f64 {
    let x = sorted(sample.to_vec());
    let cdf: Vec<f64> = x.iter().map(|&v| f.cdf(v)).collect();
    ks_pvalue(ks_statistic(&cdf), x.len())
}

fn rounded(v: &[f64], d: usize) -> Vec<f64> { v.iter().map(|x| round_to(*x, d)).collect() }

fn kfold(sigma: &[f64], dist: Distribution, s: &Settings, rng: &mut Rng, k: usize) -> Result<Value, String> {
    let n = sigma.len();
    let (mut ll, mut below5, mut below95, mut count) = (Vec::new(), 0usize, 0usize, 0usize);
    let mut pp = Vec::new();
    for rep in 0..VALID_K_REPEATS {
        let perm = rng.permutation(n);
        let mut pit = vec![0.0; n];
        let (base, extra) = (n / k, n % k);
        let mut start = 0;
        for part in 0..k {
            let len = base + usize::from(part < extra);
            let test = &perm[start..start + len];
            start += len;
            let mut mask = vec![true; n];
            for &t in test { mask[t] = false; }
            let train: Vec<f64> = (0..n).filter(|&i| mask[i]).map(|i| sigma[i]).collect();
            let f = fit(dist, &train, s)?;
            for &t in test {
                let x = sigma[t];
                ll.push(log_density(&f, x));
                below5 += usize::from(x < f.frak5);
                below95 += usize::from(x < f.frak5_95);
                pit[t] = f.cdf(x);
            }
            count += test.len();
        }
        if rep == 0 {
            pp = sorted(pit);
        }
    }
    let mut u5 = share_test(below5 as f64 / VALID_K_REPEATS as f64, n);
    u5["anteil"] = json!(below5 as f64 / count as f64);
    let mut u95 = share_test(below95 as f64 / VALID_K_REPEATS as f64, n);
    u95["anteil"] = json!(below95 as f64 / count as f64);
    Ok(json!({"ll": ll.iter().sum::<f64>() / ll.len() as f64, "unter5": u5, "unter95": u95, "pp": rounded(&pp, 4)}))
}

fn random_split(sigma: &[f64], dist: Distribution, s: &Settings, rng: &mut Rng, share: f64) -> Result<Value, String> {
    let n = sigma.len();
    // at least 3 values for fitting and 2 for testing
    let n_train = ((share * n as f64).round() as usize).max(3).min(n - 2);
    let (mut rejected, mut share5) = (0usize, Vec::new());
    for _ in 0..VALID_SPLIT_N {
        let idx = rng.permutation(n);
        let train: Vec<f64> = idx[..n_train].iter().map(|&i| sigma[i]).collect();
        let test: Vec<f64> = idx[n_train..].iter().map(|&i| sigma[i]).collect();
        let f = fit(dist, &train, s)?;
        if ks_p(&test, &f) < 0.05 { rejected += 1; }
        share5.push(test.iter().filter(|&&x| x < f.frak5).count() as f64 / test.len() as f64);
    }
    Ok(json!({"n_lern": n_train, "n_test": n - n_train, "abgelehnt": rejected as f64 / VALID_SPLIT_N as f64,
              "anteil5": rounded(&share5, 4), "anteil5_mittel": share5.iter().sum::<f64>() / share5.len() as f64}))
}

fn bootstrap(sigma: &[f64], dist: Distribution, s: &Settings, rng: &mut Rng, full: [f64; 4]) -> Result<Value, String> {
    let n = sigma.len();
    let mut values: [Vec<f64>; 4] = Default::default();
    for _ in 0..VALID_BOOT_N {
        let sample: Vec<f64> = (0..n).map(|_| sigma[rng.below(n)]).collect();
        let p = params(&fit(dist, &sample, s)?);
        for i in 0..4 { values[i].push(p[i]); }
    }
    let names = ["p1", "p2", "frak5", "frak5_95"];
    let mut out = serde_json::Map::new();
    for i in 0..4 {
        let v = sorted(values[i].clone());
        let q: serde_json::Map<String, Value> = [("2.5", 2.5), ("5", 5.0), ("50", 50.0), ("95", 95.0), ("97.5", 97.5)]
            .iter().map(|(k, p)| (k.to_string(), json!(percentile(&v, *p)))).collect();
        out.insert(names[i].into(), json!({"q": q, "voll": full[i]}));
    }
    out["frak5"]["werte"] = json!(rounded(&values[2], 3));
    Ok(Value::Object(out))
}

/// Anderson–Darling with parametric bootstrap: draw samples from the fitted
/// distribution, fit them the same way and compare A² (accounts for the
/// parameters being estimated from the same data).
fn goodness_of_fit(sigma: &[f64], dist: Distribution, s: &Settings, rng: &mut Rng, full: &Fit) -> Value {
    let n = sigma.len();
    let a2_obs = full.goodness_of_fit().0;
    let mut sim = Vec::new();
    for _ in 0..VALID_GOF_N {
        let sample: Vec<f64> = (0..n).map(|_| match dist {
            Distribution::Weibull => full.scale * rng.weibull(full.shape),
            Distribution::Lognormal => (full.mu + full.sd * rng.normal()).exp(),
        }).collect();
        if let Ok(f) = fit(dist, &sample, s) {
            sim.push(f.goodness_of_fit().0);
        }
    }
    let above = sim.iter().filter(|&&a| a >= a2_obs).count();
    json!({"a2": a2_obs, "p": (1 + above) as f64 / (sim.len() + 1) as f64, "sim": rounded(&sim, 4)})
}

fn leave_one_out(sigma: &[f64], info: &[String], dist: Distribution, s: &Settings, rng: &mut Rng,
                 full: [f64; 4]) -> Result<Value, String> {
    let n = sigma.len();
    let idx: Vec<usize> = if n <= VALID_LOO_MAX { (0..n).collect() } else { rng.choose_sorted(n, VALID_LOO_MAX) };
    let reference = full[3];
    let (mut delta, mut fractiles, mut p1) = (Vec::new(), Vec::new(), Vec::new());
    for &i in &idx {
        let rest: Vec<f64> = sigma.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, v)| *v).collect();
        let p = params(&fit(dist, &rest, s)?);
        fractiles.push(p[3]);
        p1.push(p[0]);
        delta.push(100.0 * (p[3] - reference) / reference);
    }
    let m = fractiles.iter().sum::<f64>() / fractiles.len() as f64;
    let nf = n as f64;
    let se = ((nf - 1.0) / nf * fractiles.iter().map(|f| (f - m).powi(2)).sum::<f64>() * nf / idx.len() as f64).sqrt();
    let fold = |v: &[f64], init: f64, g: fn(f64, f64) -> f64| v.iter().copied().fold(init, g);
    Ok(json!({
        "sigma": idx.iter().map(|&i| round_to(sigma[i], 3)).collect::<Vec<_>>(),
        "delta": rounded(&delta, 3),
        "info": idx.iter().map(|&i| info[i].clone()).collect::<Vec<_>>(),
        "max_abs": delta.iter().fold(0.0f64, |a, d| a.max(d.abs())),
        "se": se,
        "frak_min": fold(&fractiles, f64::INFINITY, f64::min), "frak_max": fold(&fractiles, f64::NEG_INFINITY, f64::max),
        "p1_min": fold(&p1, f64::INFINITY, f64::min), "p1_max": fold(&p1, f64::NEG_INFINITY, f64::max),
        "auswahl": idx.len() < n,
    }))
}

/// Per group (value of a column, e.g. pane or source): fit to all other
/// groups, predict the one left out (KS test, Holm-corrected).
fn leave_groups_out(sigma: &[f64], labels: &[String], column: &str, dist: Distribution, s: &Settings) -> Result<Value, String> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<usize>> = Default::default();
    for (i, l) in labels.iter().enumerate() {
        if ["–", "-", ""].contains(&l.as_str()) { continue; }
        groups.entry(l.clone()).or_insert_with(|| { order.push(l.clone()); Vec::new() }).push(i);
    }
    let too_small = order.iter().filter(|g| groups[*g].len() < 3).count();
    let mut keep: Vec<&String> = order.iter().filter(|g| groups[*g].len() >= 3).collect();
    if keep.len() < 2 {
        return Ok(json!({"fehler_code": "valid_gruppen", "spalte": column}));
    }
    keep.sort_by(|a, b| groups[*b].len().cmp(&groups[*a].len()));
    keep.truncate(VALID_MAX_GROUPS);
    let mut out = Vec::new();
    for name in keep {
        let g = &groups[name];
        let rest: Vec<f64> = (0..sigma.len()).filter(|i| !g.contains(i)).map(|i| sigma[i]).collect();
        if rest.len() < 3 { continue; }
        let f = fit(dist, &rest, s)?;
        let x: Vec<f64> = g.iter().map(|&i| sigma[i]).collect();
        let nx = x.len() as f64;
        let model_median = match dist {
            Distribution::Weibull => f.scale * 2f64.ln().powf(1.0 / f.shape),
            Distribution::Lognormal => f.mu.exp(),
        };
        let xs = sorted(x.clone());
        let med = if xs.len() % 2 == 1 { xs[xs.len() / 2] } else { 0.5 * (xs[xs.len() / 2 - 1] + xs[xs.len() / 2]) };
        out.push(json!({"gruppe": name, "n": x.len(),
            "unter5": x.iter().filter(|&&v| v < f.frak5).count() as f64 / nx,
            "unter95": x.iter().filter(|&&v| v < f.frak5_95).count() as f64 / nx,
            "median_verh": med / model_median, "p": ks_p(&x, &f)}));
    }
    let ps: Vec<f64> = out.iter().map(|g| g["p"].as_f64().unwrap()).collect();
    for (g, p) in out.iter_mut().zip(holm(&ps)) {
        g["p_holm"] = json!(p);
    }
    Ok(json!({"spalte": column, "gruppen": out, "zu_klein": too_small}))
}

pub fn validate(data: &Data, req: &Value) -> Value {
    let (settings, target) = read_settings(req);
    let series = req.get("serie").cloned().unwrap_or(json!({}));
    let name = series_name(&series);
    let k = req.get("k").and_then(|v| v.as_f64()).map(|v| v as usize).filter(|&v| v > 0).unwrap_or(5).max(2);
    let share = req.get("split_anteil").and_then(|v| v.as_f64()).unwrap_or(VALID_SPLIT_SHARE);
    let share = if VALID_SPLIT_SHARES.iter().any(|a| (a - share).abs() < 1e-9) { share } else { VALID_SPLIT_SHARE };
    let mut column = req.get("gruppen_spalte").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !column.is_empty() && data.column_index(&column).is_none() {
        column.clear();
    }
    let hover: Vec<usize> = req.get("hover_spalten").and_then(|v| v.as_array()).map(|a| a.iter()
        .filter_map(|c| c.as_str().and_then(|c| data.column_index(c))).collect()).unwrap_or_default();
    let w = series_values(data, &series, target, settings.method, settings.positions);
    let n = w.rows.len();
    if n < VALID_MIN_N {
        return json!({"fehler_code": "valid_zu_wenig", "details": {"name": name, "n": n, "min": VALID_MIN_N}});
    }
    let k = k.min(n);
    let info: Vec<String> = w.rows.iter().map(|&r| hover.iter()
        .filter(|&&c| !["–", "-"].contains(&data.rows[r].display[c].as_str()))
        .map(|&c| format!("{}: {}", data.columns[c].name, data.rows[r].display[c]))
        .collect::<Vec<_>>().join("<br>")).collect();
    let labels: Vec<String> = match data.column_index(&column) {
        Some(c) => w.rows.iter().map(|&r| data.rows[r].display[c].clone()).collect(),
        None => vec![],
    };
    let run = |dist: Distribution| -> Result<Value, String> {
        let mut rng = Rng::new(VALID_SEED);
        let full_fit = fit(dist, &w.sigma, &settings)?;
        let full = params(&full_fit);
        let mut e = json!({
            "voll": {"p1": full[0], "p2": full[1], "frak5": full[2], "frak5_95": full[3]},
            "kfold": kfold(&w.sigma, dist, &settings, &mut rng, k)?,
            "loo": leave_one_out(&w.sigma, &info, dist, &settings, &mut rng, full)?,
            "split": random_split(&w.sigma, dist, &settings, &mut rng, share)?,
            "bootstrap": bootstrap(&w.sigma, dist, &settings, &mut rng, full)?,
            "gof": goodness_of_fit(&w.sigma, dist, &settings, &mut rng, &full_fit),
        });
        if !column.is_empty() {
            e["gruppen"] = leave_groups_out(&w.sigma, &labels, &column, dist, &settings)?;
        }
        Ok(e)
    };
    let (weibull, lognormal) = match (run(Distribution::Weibull), run(Distribution::Lognormal)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(t), _) | (_, Err(t)) => return json!({"fehler_code": "komb_rechnung", "details": {"name": name, "text": t}}),
    };
    let hints: Vec<Value> = w.hints.iter().map(|h| {
        let mut h = h.as_object().cloned().unwrap_or_default();
        h.insert("name".into(), json!(name));
        Value::Object(h)
    }).collect();
    json!({"name": name, "n": n, "k": k, "gruppen_spalte": column, "umrechnung": w.conversion.is_some(),
           "hinweise": hints, "wdh": VALID_K_REPEATS, "split_n": VALID_SPLIT_N, "split_anteil": share,
           "boot_n": VALID_BOOT_N, "gof_n": VALID_GOF_N, "loo_max": VALID_LOO_MAX,
           "arten": {"weibull": weibull, "lognormal": lognormal}})
}

#[cfg(test)]
mod seed_spread {
    use super::*;
    /// How much does the hold-out rejection rate vary with the seed alone?
    #[test]
    #[ignore]
    fn split_rejection_spread() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/data/");
        let files: Vec<_> = ["synth_a.xlsx", "synth_b.xlsx"].iter()
            .map(|n| crate::read_excel(&std::fs::read(format!("{dir}{n}")).unwrap(), n).unwrap()).collect();
        let data = crate::build_data(&files);
        let req = json!({"methode": "m3_glue", "plotpositionen": "median", "lognormal_methode": "moment"});
        let (s, _) = read_settings(&req);
        let w = series_values(&data, &json!({"filter": {"Source": ["lab a"]}}), 0.0, s.method, s.positions);
        let rates: Vec<f64> = (1..=20).map(|seed| {
            let mut rng = Rng::new(seed);
            random_split(&w.sigma, Distribution::Lognormal, &s, &mut rng, 0.5).unwrap()["abgelehnt"].as_f64().unwrap()
        }).collect();
        let m = rates.iter().sum::<f64>() / rates.len() as f64;
        let sd = (rates.iter().map(|r| (r - m).powi(2)).sum::<f64>() / (rates.len() - 1) as f64).sqrt();
        println!("rates {:?}\nmean {m:.3} sd {sd:.3} min {:.3} max {:.3}", rates,
                 rates.iter().cloned().fold(1.0, f64::min), rates.iter().cloned().fold(0.0, f64::max));
    }
}
