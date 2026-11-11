//! One-sample Kolmogorov–Smirnov test (two-sided) against a fully specified
//! distribution, with the exact distribution of D (Marsaglia, Tsang & Wang
//! 2003, "Evaluating Kolmogorov's distribution") and their asymptotic
//! approximation far in the tail.

/// D statistic of sorted data with the CDF values `cdf_sorted`.
pub fn ks_statistic(cdf_sorted: &[f64]) -> f64 {
    let n = cdf_sorted.len() as f64;
    cdf_sorted.iter().enumerate().fold(0.0f64, |d, (i, &f)| {
        let i = i as f64;
        d.max((i + 1.0) / n - f).max(f - i / n)
    })
}

/// Two-sided p-value P(D ≥ d) for sample size n.
pub fn ks_pvalue(d: f64, n: usize) -> f64 {
    if d <= 0.0 {
        return 1.0;
    }
    if d >= 1.0 {
        return 0.0;
    }
    let nf = n as f64;
    let s = d * d * nf;
    if s > 7.24 || (s > 3.76 && n > 99) {
        return (2.0 * (-(2.000071 + 0.331 / nf.sqrt() + 1.409 / nf) * s).exp()).min(1.0);
    }
    (1.0 - kolmogorov_cdf(n, d)).clamp(0.0, 1.0)
}

/// Exact P(D_n < d) after Marsaglia, Tsang & Wang (2003).
fn kolmogorov_cdf(n: usize, d: f64) -> f64 {
    let nf = n as f64;
    let k = (nf * d) as usize + 1;
    let m = 2 * k - 1;
    let h = k as f64 - nf * d;
    let mut hm = vec![0.0f64; m * m];
    for i in 0..m {
        for j in 0..m {
            hm[i * m + j] = if (i as isize) - (j as isize) + 1 < 0 { 0.0 } else { 1.0 };
        }
    }
    for i in 0..m {
        hm[i * m] -= h.powi(i as i32 + 1);
        hm[(m - 1) * m + i] -= h.powi((m - i) as i32);
    }
    if 2.0 * h - 1.0 > 0.0 {
        hm[(m - 1) * m] += (2.0 * h - 1.0).powi(m as i32);
    }
    for i in 0..m {
        for j in 0..m {
            let diff = i as isize - j as isize + 1;
            if diff > 0 {
                for g in 1..=diff {
                    hm[i * m + j] /= g as f64;
                }
            }
        }
    }
    let (q, mut eq) = mat_power(&hm, 0, m, n);
    let mut s = q[(k - 1) * m + k - 1];
    for i in 1..=n {
        s = s * i as f64 / nf;
        if s < 1e-140 {
            s *= 1e140;
            eq -= 140;
        }
    }
    s * 10f64.powi(eq)
}

fn mat_mul(a: &[f64], b: &[f64], m: usize) -> Vec<f64> {
    let mut c = vec![0.0; m * m];
    for i in 0..m {
        for k in 0..m {
            let aik = a[i * m + k];
            if aik == 0.0 {
                continue;
            }
            for j in 0..m {
                c[i * m + j] += aik * b[k * m + j];
            }
        }
    }
    c
}

/// A^n with a separate decimal exponent to avoid overflow.
fn mat_power(a: &[f64], ea: i32, m: usize, n: usize) -> (Vec<f64>, i32) {
    if n == 1 {
        return (a.to_vec(), ea);
    }
    let (half, eh) = mat_power(a, ea, m, n / 2);
    let mut b = mat_mul(&half, &half, m);
    let mut eb = 2 * eh;
    if n % 2 == 1 {
        b = mat_mul(a, &b, m);
        eb += ea;
    }
    if b[(m / 2) * m + m / 2] > 1e140 {
        for v in b.iter_mut() {
            *v *= 1e-140;
        }
        eb += 140;
    }
    (b, eb)
}
