//! Statistical tests: Pearson correlation, Kruskal–Wallis, Dunn's post-hoc
//! test with Holm/Bonferroni correction, compact letter display, and the
//! k-sample Anderson–Darling test (Scholz & Stephens 1987).

use statrs::function::{beta::beta_reg, erf::erfc, gamma::gamma_ur};

fn mean(v: &[f64]) -> f64 { v.iter().sum::<f64>() / v.len() as f64 }

/// Upper tail of the standard normal distribution.
pub fn norm_sf(z: f64) -> f64 { 0.5 * erfc(z / std::f64::consts::SQRT_2) }

/// Upper tail of the chi-square distribution.
pub fn chi2_sf(x: f64, df: f64) -> f64 {
    if x <= 0.0 { 1.0 } else { gamma_ur(df / 2.0, x / 2.0) }
}

/// Pearson correlation r and its two-sided p-value (exact, via the beta
/// distribution of r under independence).
pub fn pearson(x: &[f64], y: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let (xm, ym) = (mean(x), mean(y));
    let dx: Vec<f64> = x.iter().map(|v| v - xm).collect();
    let dy: Vec<f64> = y.iter().map(|v| v - ym).collect();
    let nx = dx.iter().map(|v| v * v).sum::<f64>().sqrt();
    let ny = dy.iter().map(|v| v * v).sum::<f64>().sqrt();
    let r = dx.iter().zip(&dy).map(|(a, b)| (a / nx) * (b / ny)).sum::<f64>().clamp(-1.0, 1.0);
    let ab = n / 2.0 - 1.0;
    let p = if r.abs() >= 1.0 { 0.0 } else { (2.0 * beta_reg(ab, ab, (1.0 - r.abs()) / 2.0)).min(1.0) };
    (r, p)
}

/// Exact two-sided binomial test of k successes in n trials against p:
/// the probability of all outcomes that are at most as likely as k.
pub fn binom_test(k: usize, n: usize, p: f64) -> f64 {
    use statrs::function::gamma::ln_gamma;
    let nf = n as f64;
    let ln_pmf = |j: usize| {
        let j = j as f64;
        ln_gamma(nf + 1.0) - ln_gamma(j + 1.0) - ln_gamma(nf - j + 1.0) + j * p.ln() + (nf - j) * (1.0 - p).ln()
    };
    if (k as f64 - p * nf).abs() < 1e-12 {
        return 1.0;
    }
    let d = ln_pmf(k).exp() * (1.0 + 1e-7);
    let total: f64 = (0..=n).map(|j| ln_pmf(j).exp()).filter(|&q| q <= d).sum();
    total.min(1.0)
}

/// Percentile with linear interpolation between order statistics (as numpy).
pub fn percentile(sorted: &[f64], q: f64) -> f64 {
    let pos = q / 100.0 * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    sorted[lo] + (pos - lo as f64) * (sorted[hi] - sorted[lo])
}

/// Ranks 1…N with ties sharing the average rank.
pub fn rankdata(v: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|&a, &b| v[a].partial_cmp(&v[b]).unwrap());
    let mut ranks = vec![0.0; v.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[i]] {
            j += 1;
        }
        let r = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            ranks[idx[k]] = r;
        }
        i = j + 1;
    }
    ranks
}

/// Σ(t³ − t) over groups of tied values.
fn tie_sum(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut sum = 0.0;
    let mut i = 0;
    while i < s.len() {
        let mut j = i;
        while j + 1 < s.len() && s[j + 1] == s[i] {
            j += 1;
        }
        let t = (j - i + 1) as f64;
        sum += t * t * t - t;
        i = j + 1;
    }
    sum
}

/// Kruskal–Wallis H (tie-corrected) and p-value.
pub fn kruskal(groups: &[Vec<f64>]) -> (f64, f64) {
    let all: Vec<f64> = groups.iter().flatten().copied().collect();
    let n = all.len() as f64;
    let ranks = rankdata(&all);
    let mut start = 0;
    let mut h = 0.0;
    for g in groups {
        let s: f64 = ranks[start..start + g.len()].iter().sum();
        h += s * s / g.len() as f64;
        start += g.len();
    }
    h = 12.0 / (n * (n + 1.0)) * h - 3.0 * (n + 1.0);
    h /= 1.0 - tie_sum(&all) / (n * n * n - n);
    (h, chi2_sf(h, groups.len() as f64 - 1.0))
}

/// Holm step-down adjustment of p-values.
pub fn holm(p: &[f64]) -> Vec<f64> {
    let m = p.len();
    let mut idx: Vec<usize> = (0..m).collect();
    idx.sort_by(|&a, &b| p[a].partial_cmp(&p[b]).unwrap());
    let mut out = vec![0.0; m];
    let mut running: f64 = 0.0;
    for (rank, &i) in idx.iter().enumerate() {
        running = running.max(((m - rank) as f64 * p[i]).min(1.0));
        out[i] = running;
    }
    out
}

#[derive(Clone, Debug)]
pub struct DunnPair {
    pub i: usize,
    pub j: usize,
    pub z: f64,
    pub p_raw: f64,
    pub p: f64,
}

/// Dunn's test: every pair compared by mean ranks of the common ranking, with
/// tie correction. Returns the mean ranks and the pairs (i < j).
pub fn dunn(groups: &[Vec<f64>], bonferroni: bool) -> (Vec<f64>, Vec<DunnPair>) {
    let all: Vec<f64> = groups.iter().flatten().copied().collect();
    let n = all.len() as f64;
    let ranks = rankdata(&all);
    let mut mean_ranks = Vec::new();
    let mut start = 0;
    for g in groups {
        mean_ranks.push(mean(&ranks[start..start + g.len()]));
        start += g.len();
    }
    let ties = tie_sum(&all) / (12.0 * (n - 1.0));
    let mut pairs = Vec::new();
    for i in 0..groups.len() {
        for j in i + 1..groups.len() {
            let se = ((n * (n + 1.0) / 12.0 - ties)
                * (1.0 / groups[i].len() as f64 + 1.0 / groups[j].len() as f64)).sqrt();
            let z = (mean_ranks[i] - mean_ranks[j]) / se;
            pairs.push(DunnPair { i, j, z, p_raw: 2.0 * norm_sf(z.abs()), p: 0.0 });
        }
    }
    let raw: Vec<f64> = pairs.iter().map(|p| p.p_raw).collect();
    let adj: Vec<f64> = if bonferroni {
        raw.iter().map(|p| (p * raw.len() as f64).min(1.0)).collect()
    } else {
        holm(&raw)
    };
    for (p, a) in pairs.iter_mut().zip(adj) {
        p.p = a;
    }
    (mean_ranks, pairs)
}

/// Compact letter display (insert-and-absorb, Piepho 2004): groups sharing a
/// letter show no detectable difference. `ranking` lists group indices from
/// the highest median down; "a" goes to the first.
pub fn letters(k: usize, pairs: &[DunnPair], alpha: f64, ranking: &[usize]) -> Vec<String> {
    let mut columns: Vec<Vec<usize>> = vec![(0..k).collect()];
    for p in pairs.iter().filter(|p| p.p < alpha) {
        let mut fresh: Vec<Vec<usize>> = Vec::new();
        for s in &columns {
            if s.contains(&p.i) && s.contains(&p.j) {
                fresh.push(s.iter().copied().filter(|&g| g != p.i).collect());
                fresh.push(s.iter().copied().filter(|&g| g != p.j).collect());
            } else {
                fresh.push(s.clone());
            }
        }
        // absorb: drop columns that are contained in another one
        fresh.sort_by(|a, b| b.len().cmp(&a.len()));
        columns = Vec::new();
        for s in fresh {
            if !s.is_empty() && !columns.iter().any(|t: &Vec<usize>| s.iter().all(|g| t.contains(g))) {
                columns.push(s);
            }
        }
    }
    let pos = |g: usize| ranking.iter().position(|&r| r == g).unwrap();
    columns.sort_by_key(|s| s.iter().map(|&g| pos(g)).min().unwrap());
    let mut out = vec![String::new(); k];
    for (nr, s) in columns.iter().enumerate() {
        let ch = if nr < 26 { ((b'a' + nr as u8) as char).to_string() } else { format!("[{}]", nr + 1) };
        for &g in s {
            out[g].push_str(&ch);
        }
    }
    out
}

/// k-sample Anderson–Darling test (Scholz & Stephens 1987, midrank version for
/// ties). Returns the standardised statistic and the p-value, which is
/// interpolated from the published critical values and capped to 0.001 … 0.25.
pub fn anderson_ksamp(samples: &[Vec<f64>]) -> (f64, f64) {
    let k = samples.len();
    let mut z: Vec<f64> = samples.iter().flatten().copied().collect();
    z.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let nn = z.len();
    let n_total = nn as f64;
    let mut zstar = z.clone();
    zstar.dedup();
    let left = |v: &[f64], x: f64| v.partition_point(|&a| a < x) as f64;
    let right = |v: &[f64], x: f64| v.partition_point(|&a| a <= x) as f64;
    let lj: Vec<f64> = zstar.iter().map(|&x| if nn == zstar.len() { 1.0 } else { right(&z, x) - left(&z, x) }).collect();
    let bj: Vec<f64> = zstar.iter().zip(&lj).map(|(&x, l)| left(&z, x) + l / 2.0).collect();
    let mut a2 = 0.0;
    for s in samples {
        let mut s = s.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let ni = s.len() as f64;
        let mut inner = 0.0;
        for (j, &x) in zstar.iter().enumerate() {
            let fij = right(&s, x) - left(&s, x);
            let mij = right(&s, x) - fij / 2.0;
            inner += lj[j] / n_total * (n_total * mij - bj[j] * ni).powi(2)
                / (bj[j] * (n_total - bj[j]) - n_total * lj[j] / 4.0);
        }
        a2 += inner / ni;
    }
    a2 *= (n_total - 1.0) / n_total;

    let h_small: f64 = samples.iter().map(|s| 1.0 / s.len() as f64).sum();
    let hs: Vec<f64> = (2..nn).rev().map(|v| 1.0 / v as f64).scan(0.0, |acc, v| { *acc += v; Some(*acc) }).collect();
    let h = hs.last().copied().unwrap_or(0.0) + 1.0;
    let g: f64 = hs.iter().enumerate().map(|(i, v)| v / (i + 2) as f64).sum();
    let kf = k as f64;
    let a = (4.0 * g - 6.0) * (kf - 1.0) + (10.0 - 6.0 * g) * h_small;
    let b = (2.0 * g - 4.0) * kf * kf + 8.0 * h * kf + (2.0 * g - 14.0 * h - 4.0) * h_small - 8.0 * h + 4.0 * g - 6.0;
    let c = (6.0 * h + 2.0 * g - 2.0) * kf * kf + (4.0 * h - 4.0 * g + 6.0) * kf + (2.0 * h - 6.0) * h_small + 4.0 * h;
    let d = (2.0 * h + 6.0) * kf * kf - 4.0 * h * kf;
    let sigmasq = (a * n_total.powi(3) + b * n_total.powi(2) + c * n_total + d)
        / ((n_total - 1.0) * (n_total - 2.0) * (n_total - 3.0));
    let m = kf - 1.0;
    let stat = (a2 - m) / sigmasq.sqrt();

    const B0: [f64; 7] = [0.675, 1.281, 1.645, 1.96, 2.326, 2.573, 3.085];
    const B1: [f64; 7] = [-0.245, 0.25, 0.678, 1.149, 1.822, 2.364, 3.615];
    const B2: [f64; 7] = [-0.105, -0.305, -0.362, -0.391, -0.396, -0.345, -0.154];
    const SIG: [f64; 7] = [0.25, 0.1, 0.05, 0.025, 0.01, 0.005, 0.001];
    let crit: Vec<f64> = (0..7).map(|i| B0[i] + B1[i] / m.sqrt() + B2[i] / m).collect();
    let p = if stat < crit.iter().cloned().fold(f64::INFINITY, f64::min) {
        0.25
    } else if stat > crit.iter().cloned().fold(f64::NEG_INFINITY, f64::max) {
        0.001
    } else {
        // quadratic least-squares fit of ln(sig) on the critical values
        let ys: Vec<f64> = SIG.iter().map(|s| s.ln()).collect();
        let coef = polyfit2(&crit, &ys);
        (coef[0] * stat * stat + coef[1] * stat + coef[2]).exp()
    };
    (stat, p)
}

/// Least-squares parabola y = c0·x² + c1·x + c2 (as numpy.polyfit(x, y, 2)).
fn polyfit2(x: &[f64], y: &[f64]) -> [f64; 3] {
    // normal equations, solved by Gaussian elimination with pivoting
    let mut m = [[0.0f64; 4]; 3];
    for (&xi, &yi) in x.iter().zip(y) {
        let p = [xi * xi, xi, 1.0];
        for r in 0..3 {
            for c in 0..3 {
                m[r][c] += p[r] * p[c];
            }
            m[r][3] += p[r] * yi;
        }
    }
    for col in 0..3 {
        let piv = (col..3).max_by(|&a, &b| m[a][col].abs().partial_cmp(&m[b][col].abs()).unwrap()).unwrap();
        m.swap(col, piv);
        for r in 0..3 {
            if r != col {
                let f = m[r][col] / m[col][col];
                for c in col..4 {
                    m[r][c] -= f * m[col][c];
                }
            }
        }
    }
    [m[0][3] / m[0][0], m[1][3] / m[1][1], m[2][3] / m[2][2]]
}
