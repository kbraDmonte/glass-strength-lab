//! Quantile and distribution functions used by the fits.

use statrs::distribution::{ChiSquared, ContinuousCDF, Normal, StudentsT};

/// Standard normal quantile Φ⁻¹(p).
pub fn norm_ppf(p: f64) -> f64 {
    Normal::new(0.0, 1.0).unwrap().inverse_cdf(p)
}

/// Standard normal distribution function Φ(z).
pub fn norm_cdf(z: f64) -> f64 {
    Normal::new(0.0, 1.0).unwrap().cdf(z)
}

/// Student's t quantile with `df` degrees of freedom.
pub fn t_ppf(p: f64, df: f64) -> f64 {
    StudentsT::new(0.0, 1.0, df).unwrap().inverse_cdf(p)
}

/// Chi-square quantile with `df` degrees of freedom.
pub fn chi2_ppf(p: f64, df: f64) -> f64 {
    ChiSquared::new(df).unwrap().inverse_cdf(p)
}

/// Weibull paper ordinate: y = ln(−ln(1 − P)).
pub fn weibull_y(p: f64) -> f64 {
    (-(1.0 - p).ln()).ln()
}

/// Ordinate of the probability paper of a distribution.
pub fn paper_y(dist: crate::Distribution, p: f64) -> f64 {
    match dist {
        crate::Distribution::Weibull => weibull_y(p),
        crate::Distribution::Lognormal => norm_ppf(p),
    }
}
