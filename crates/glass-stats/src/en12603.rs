//! EN 12603 (Good Linear Unbiased Estimator, two-parameter Weibull),
//! implemented after the open-GLASSlab TeachBook, method 3 (CC BY 4.0).

/// κₙ for n = 2 … 60 (EN 12603, Table 3); 1.5692 for n > 60.
const KAPPA: [f64; 59] = [
    0.6931, 0.9808, 1.1507, 1.2674, 1.3545, 1.1828, 1.2547, 1.3141, 1.3644, 1.4079, 1.4461,
    1.3332, 1.3686, 1.4004, 1.4293, 1.4556, 1.4799, 1.3960, 1.4192, 1.4408, 1.4609, 1.4797,
    1.4975, 1.5142, 1.4479, 1.4642, 1.4796, 1.4943, 1.5083, 1.5216, 1.4665, 1.4795, 1.4920,
    1.5040, 1.5156, 1.5266, 1.4795, 1.4904, 1.5009, 1.5110, 1.5208, 1.5303, 1.4891, 1.4984,
    1.5075, 1.5163, 1.5248, 1.5331, 1.5411, 1.5046, 1.5126, 1.5204, 1.5279, 1.5352, 1.5424,
    1.5096, 1.5167, 1.5236, 1.5304,
];

pub fn kappa(n: usize) -> Option<f64> {
    match n {
        0 | 1 => None,
        2..=60 => Some(KAPPA[n - 2]),
        _ => Some(1.5692),
    }
}

/// Shape β̂ and scale θ̂ from the sorted sample.
pub fn estimate(sorted: &[f64]) -> Option<(f64, f64)> {
    let n = sorted.len();
    let k = kappa(n)?;
    let s = (0.84 * n as f64).floor() as usize;
    if s == 0 || s >= n {
        return None;
    }
    let ln: Vec<f64> = sorted.iter().map(|v| v.ln()).collect();
    let upper: f64 = ln[s..].iter().sum();
    let lower: f64 = ln[..s].iter().sum();
    let nf = n as f64;
    let beta = nf * k / ((s as f64 / (n - s) as f64) * upper - lower);
    let theta = (ln.iter().sum::<f64>() / nf + 0.5772 / beta).exp();
    Some((beta, theta))
}

/// Degrees of freedom f₁ for the confidence interval of β (uncensored sample).
pub fn f1(n: usize) -> f64 {
    let nf = n as f64;
    nf * (3.085 - 3.84 / nf)
}

/// Bounds of the probability of failure G(x) at stress `x`:
/// returns (G_ob, G_un) for the significance level `alpha`.
pub fn g_bounds(n: usize, beta: f64, theta: f64, x: f64, alpha: f64) -> (f64, f64) {
    let nf = n as f64;
    let y = beta * (theta / x).ln();
    let a = 1.162 / nf;
    let b = 0.6482 / nf + 0.805 / nf.powi(2) + 1.13 / nf.powi(3);
    let c = -0.2309 / nf + 0.15 / nf.powi(2) + 1.78 / nf.powi(3);
    // the standard defines the piecewise functions for 0 <= v <= 5 only;
    // clamp, so that a single extreme value does not stop the evaluation
    let v = (a + b * y * y - 2.0 * c * y).clamp(0.0, 5.0);
    let (f2, h) = if v <= 2.0 {
        let f2 = (8.0 * v + 12.0) / (v * v + 6.0 * v);
        (f2, (15.0 * f2 * f2 + 5.0 * f2 + 6.0) / (15.0 * f2.powi(3) + 6.0 * f2))
    } else {
        (3.509 - 1.3055 * v + 0.2480 * v * v - 0.0175 * v.powi(3),
         0.08832 + 0.3218 * v - 0.0167 * v * v)
    };
    let gamma = (-y + h).exp();
    let g_ob = 1.0 - (-gamma * crate::dist::chi2_ppf(1.0 - alpha / 2.0, f2) / f2).exp();
    let g_un = 1.0 - (-gamma * crate::dist::chi2_ppf(alpha / 2.0, f2) / f2).exp();
    (g_ob, g_un)
}
