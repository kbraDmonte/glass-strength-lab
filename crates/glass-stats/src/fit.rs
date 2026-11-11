//! Fits on probability paper.
//!
//! Weibull paper:   x = ln σ, y = ln(−ln(1 − P)); line y = m·x + b, β = exp(−b/m).
//! Log-normal paper: x = ln σ, y = Φ⁻¹(P);      line y = (x − μ)/s.
//!
//! The confidence band of the line is
//!   C(x) = t · s_yx · √(1/n + (x − x̄)² / Sxx),
//! with t = t(0.95; n−2) (one-sided) or t(0.975; n−2) (two-sided) and s_yx the
//! standard error of an ordinary least-squares fit of y on x. The lower bound
//! (towards lower strength) is line + C, the upper bound line − C. For EN 12603
//! (method 3) the band is given by the bounds G_ob/G_un of the standard instead.

use crate::{dist, en12603, gof};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Distribution { Weibull, Lognormal }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeibullMethod { LeastSquares, WeightedLeastSquares, Glue }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlottingPositions { Hazen, Mean, Median }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sides { One, Two }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LognormalMethod { Moments, Regression }

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub positions: PlottingPositions,
    pub sides: Sides,
    pub method: WeibullMethod,
    pub lognormal: LognormalMethod,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { positions: PlottingPositions::Hazen, sides: Sides::Two,
                   method: WeibullMethod::LeastSquares, lognormal: LognormalMethod::Moments }
    }
}

/// Probability of failure assigned to the i-th of n sorted specimens.
pub fn plotting_positions(n: usize, kind: PlottingPositions) -> Vec<f64> {
    let nf = n as f64;
    (1..=n).map(|i| {
        let i = i as f64;
        match kind {
            PlottingPositions::Hazen => (i - 0.5) / nf,
            PlottingPositions::Mean => i / (nf + 1.0),
            PlottingPositions::Median => (i - 0.3) / (nf + 0.4),
        }
    }).collect()
}

/// EN 12603 quantities kept for the band and the fractile bounds.
#[derive(Clone, Debug)]
pub struct GlueInfo {
    pub alpha: f64,
    pub beta_lower: f64,
    pub beta_upper: f64,
}

/// Result of one fit. Field names follow the meaning, not the old script.
#[derive(Clone, Debug)]
pub struct Fit {
    pub dist: Distribution,
    pub n: usize,
    /// sorted measurements
    pub sigma: Vec<f64>,
    pub x: Vec<f64>,
    pub p: Vec<f64>,
    pub y: Vec<f64>,
    pub slope: f64,
    pub intercept: f64,
    pub r2: f64,
    /// standard error of an OLS fit of y on x (Excel STEYX)
    pub se: f64,
    pub x_mean: f64,
    pub sxx: f64,
    /// t-quantile of the band
    pub t: f64,
    /// Weibull: shape m and scale β; log-normal: μ and s
    pub shape: f64,
    pub scale: f64,
    pub mu: f64,
    pub sd: f64,
    pub glue: Option<GlueInfo>,
    /// ln of the 5 % fractile
    pub x0: f64,
    pub frak5: f64,
    pub frak5_95: f64,
    pub frak5_ob: f64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    /// sample standard deviation of the measurements (n − 1)
    pub std: f64,
}

fn mean(v: &[f64]) -> f64 { v.iter().sum::<f64>() / v.len() as f64 }

/// slope, intercept of an ordinary least-squares fit of y on x
fn ols(x: &[f64], y: &[f64]) -> (f64, f64) {
    let (xm, ym) = (mean(x), mean(y));
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - xm) * (b - ym)).sum();
    let sxx: f64 = x.iter().map(|a| (a - xm).powi(2)).sum();
    let slope = sxy / sxx;
    (slope, ym - slope * xm)
}

fn steyx(x: &[f64], y: &[f64]) -> f64 {
    let (m, b) = ols(x, y);
    let ss: f64 = x.iter().zip(y).map(|(a, v)| (v - (m * a + b)).powi(2)).sum();
    (ss / (x.len() as f64 - 2.0)).sqrt()
}

fn r_squared(x: &[f64], y: &[f64], slope: f64, intercept: f64) -> f64 {
    let ym = mean(y);
    let ss_res: f64 = x.iter().zip(y).map(|(a, v)| (v - (slope * a + intercept)).powi(2)).sum();
    let ss_tot: f64 = y.iter().map(|v| (v - ym).powi(2)).sum();
    1.0 - ss_res / ss_tot
}

fn t_quantile(n: usize, sides: Sides) -> f64 {
    let p = if sides == Sides::One { 0.95 } else { 0.975 };
    dist::t_ppf(p, n as f64 - 2.0)
}

fn base(sorted: &[f64], dist: Distribution, settings: &Settings) -> Fit {
    let n = sorted.len();
    let x: Vec<f64> = sorted.iter().map(|v| v.ln()).collect();
    let p = plotting_positions(n, settings.positions);
    let y: Vec<f64> = p.iter().map(|&q| dist::paper_y(dist, q)).collect();
    let x_mean = mean(&x);
    let sxx = x.iter().map(|a| (a - x_mean).powi(2)).sum();
    let m = mean(sorted);
    let std = (sorted.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n as f64 - 1.0)).sqrt();
    Fit {
        dist, n, sigma: sorted.to_vec(), se: steyx(&x, &y), x, p, y,
        slope: f64::NAN, intercept: f64::NAN, r2: f64::NAN, x_mean, sxx,
        t: t_quantile(n, settings.sides), shape: f64::NAN, scale: f64::NAN,
        mu: f64::NAN, sd: f64::NAN, glue: None, x0: f64::NAN, frak5: f64::NAN,
        frak5_95: f64::NAN, frak5_ob: f64::NAN, min: sorted[0], max: sorted[n - 1], mean: m, std,
    }
}

/// Two-parameter Weibull fit of the sorted sample (n ≥ 3).
pub fn fit_weibull(sorted: &[f64], settings: &Settings) -> Result<Fit, String> {
    let mut f = base(sorted, Distribution::Weibull, settings);
    let (x, y, p) = (&f.x, &f.y, &f.p);
    let (slope, intercept) = match settings.method {
        WeibullMethod::LeastSquares => ols(x, y),
        WeibullMethod::WeightedLeastSquares => {
            // weights after Faucher & Tyson, weighted normal equations
            let w: Vec<f64> = p.iter().map(|&q| 3.3 * q - 27.5 * (1.0 - (1.0 - q).powf(0.025))).collect();
            let sw: f64 = w.iter().sum();
            let swx: f64 = w.iter().zip(x).map(|(a, b)| a * b).sum();
            let swy: f64 = w.iter().zip(y).map(|(a, b)| a * b).sum();
            let swxy: f64 = (0..x.len()).map(|i| w[i] * x[i] * y[i]).sum();
            let swxx: f64 = (0..x.len()).map(|i| w[i] * x[i] * x[i]).sum();
            let m = (sw * swxy - swx * swy) / (sw * swxx - swx * swx);
            let theta = (-(1.0 / m) * (swy - m * swx) / sw).exp();
            (m, -m * theta.ln())
        }
        WeibullMethod::Glue => {
            let (beta_hat, theta) = en12603::estimate(sorted)
                .ok_or_else(|| format!("EN 12603: no κₙ for n = {}", sorted.len()))?;
            (beta_hat, -beta_hat * theta.ln())
        }
    };
    f.slope = slope;
    f.intercept = intercept;
    f.shape = slope;
    f.scale = (-intercept / slope).exp();
    f.r2 = r_squared(x, y, slope, intercept);
    let y5 = dist::weibull_y(0.05);
    f.x0 = (y5 - intercept) / slope;
    f.frak5 = f.x0.exp();
    if settings.method == WeibullMethod::Glue {
        let alpha = if settings.sides == Sides::One { 0.10 } else { 0.05 };
        let f1 = en12603::f1(f.n);
        let beta_lower = slope * dist::chi2_ppf(alpha / 2.0, f1) / f1;
        let beta_upper = slope * dist::chi2_ppf(1.0 - alpha / 2.0, f1) / f1;
        f.glue = Some(GlueInfo { alpha, beta_lower, beta_upper });
        // EN 12603, 8.4.1: with G = 0.05 the support value x₁ is the fractile itself
        let x1 = f.frak5;
        let (g_ob, g_un) = en12603::g_bounds(f.n, slope, f.scale, x1, alpha);
        let g2 = 0.05_f64;
        f.frak5_95 = x1 * ((1.0 - g2).ln() / (1.0 - g_ob).ln()).powf(1.0 / beta_lower);
        f.frak5_ob = x1 * ((1.0 - g2).ln() / (1.0 - g_un).ln()).powf(1.0 / beta_upper);
    } else {
        f.frak5_95 = f.band_x_at(y5, true).exp();
        f.frak5_ob = f.band_x_at(y5, false).exp();
    }
    Ok(f)
}

/// Log-normal fit: μ and s by the method of moments (mean and population
/// variance of ln σ) or by regression on the log-normal paper.
pub fn fit_lognormal(sorted: &[f64], settings: &Settings) -> Fit {
    let mut f = base(sorted, Distribution::Lognormal, settings);
    let (mu, sd) = match settings.lognormal {
        LognormalMethod::Moments => {
            let mu = mean(&f.x);
            let var = f.x.iter().map(|a| (a - mu).powi(2)).sum::<f64>() / f.n as f64;
            (mu, var.sqrt())
        }
        LognormalMethod::Regression => {
            let (slope, intercept) = ols(&f.x, &f.y);
            (-intercept / slope, 1.0 / slope)
        }
    };
    f.mu = mu;
    f.sd = sd;
    f.shape = mu;
    f.scale = sd;
    f.slope = 1.0 / sd;
    f.intercept = -mu / sd;
    f.r2 = r_squared(&f.x, &f.y, f.slope, f.intercept);
    let z5 = dist::norm_ppf(0.05);
    f.x0 = mu + z5 * sd;
    f.frak5 = f.x0.exp();
    f.frak5_95 = f.band_x_at(z5, true).exp();
    f.frak5_ob = f.band_x_at(z5, false).exp();
    f
}

impl Fit {
    /// Ordinate of the fitted line at x = ln σ.
    pub fn line_y(&self, x: f64) -> f64 { self.slope * x + self.intercept }

    /// Half-width C(x) of the regression band.
    pub fn half_width(&self, x: f64) -> f64 {
        self.t * self.se * (1.0 / self.n as f64 + (x - self.x_mean).powi(2) / self.sxx).sqrt()
    }

    /// Ordinate of the lower (towards lower strength, `lower = true`) or upper
    /// confidence bound at x = ln σ.
    pub fn band_y(&self, x: f64, lower: bool) -> f64 {
        if let Some(g) = &self.glue {
            let (g_ob, g_un) = en12603::g_bounds(self.n, self.shape, self.scale, x.exp(), g.alpha);
            // keep the ordinate finite where a bound reaches 0 or 1
            let g = if lower { g_ob } else { g_un };
            return dist::weibull_y(g.clamp(1e-12, 1.0 - 1e-12));
        }
        if lower { self.line_y(x) + self.half_width(x) } else { self.line_y(x) - self.half_width(x) }
    }

    /// ln σ at which a confidence bound first reaches the level `y_target`,
    /// searching outwards from the fitted line (to lower stresses for the lower
    /// bound). If the band never reaches it (the regression is not significant
    /// enough), the point on the fitted line is returned instead.
    pub fn band_x_at(&self, y_target: f64, lower: bool) -> f64 {
        let start = (y_target - self.intercept) / self.slope;
        let f = |x: f64| self.band_y(x, lower) - y_target;
        let dir = if lower { -1.0 } else { 1.0 };
        // Close to the line the band can cross the level and come back within a
        // short distance (small samples), so scan finely first, then widen.
        let mut a = start;
        let mut dist = 0.0;
        let mut b;
        let mut tries = 0;
        loop {
            dist += if dist < 1.0 { 0.005 } else { dist };
            b = start + dir * dist;
            if f(a).signum() != f(b).signum() {
                break;
            }
            a = b;
            tries += 1;
            if tries > 260 {
                return start;
            }
        }
        let (mut lo, mut hi) = if a < b { (a, b) } else { (b, a) };
        let flo = f(lo);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            let fm = f(mid);
            if fm == 0.0 || (hi - lo) < 1e-13 {
                return mid;
            }
            if fm.signum() == flo.signum() { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    }

    /// Fitted CDF at stress σ.
    pub fn cdf(&self, sigma: f64) -> f64 {
        match self.dist {
            Distribution::Weibull => 1.0 - (-(sigma / self.scale).powf(self.shape)).exp(),
            Distribution::Lognormal => dist::norm_cdf((sigma.ln() - self.mu) / self.sd),
        }
    }

    /// (A², D, p_AD) of the fit.
    pub fn goodness_of_fit(&self) -> (f64, f64, f64) {
        let cdf: Vec<f64> = self.sigma.iter().map(|&s| self.cdf(s)).collect();
        gof::anderson_darling(&cdf)
    }

    /// Standard deviation of the fitted distribution (not of the measurements).
    pub fn std_fitted(&self) -> f64 {
        use statrs::function::gamma::gamma;
        match self.dist {
            Distribution::Weibull => {
                let (m, b) = (self.shape, self.scale);
                b * (gamma(1.0 + 2.0 / m) - gamma(1.0 + 1.0 / m).powi(2)).sqrt()
            }
            Distribution::Lognormal => {
                let (mu, s) = (self.mu, self.sd);
                (((s * s).exp() - 1.0) * (2.0 * mu + s * s).exp()).sqrt()
            }
        }
    }
}
