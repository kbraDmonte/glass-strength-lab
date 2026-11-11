//! Goodness of fit: Anderson–Darling A² with the p-value approximation used by
//! open-GLASSlab, and the Kolmogorov–Smirnov distance D.
//!
//! The parameters are estimated from the same data, so A² and p_AD are a
//! comparison measure between distributions, not a formal test (see the
//! parametric bootstrap in the validation module for a valid test).

/// (A², D, p_AD) for sorted data and the fitted CDF values at those points.
pub fn anderson_darling(cdf_sorted: &[f64]) -> (f64, f64, f64) {
    let n = cdf_sorted.len();
    let nf = n as f64;
    let mut z: Vec<f64> = cdf_sorted.iter().map(|&v| v.clamp(1e-12, 1.0 - 1e-12)).collect();
    z.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut sum = 0.0;
    let mut d: f64 = 0.0;
    for i in 0..n {
        let k = (i + 1) as f64;
        sum += (2.0 * k - 1.0) * (z[i].ln() + (1.0 - z[n - 1 - i]).ln());
        d = d.max((k / nf - z[i]).max(z[i] - (k - 1.0) / nf));
    }
    let a2 = -nf - sum / nf;
    let a_star = (1.0 + 0.2 / nf.sqrt()) * a2;
    let p = 1.0 / (1.0 + (-0.1 + 1.24 * a_star.ln() + 4.48 * a_star).exp());
    (a2, d, p)
}
