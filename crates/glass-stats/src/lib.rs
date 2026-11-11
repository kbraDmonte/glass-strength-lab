//! Strength statistics for glass.
//!
//! Two-parameter Weibull and log-normal fits on probability paper, with the
//! three Weibull methods of the TU Delft open-GLASSlab TeachBook (CC BY 4.0):
//! least squares (method 1), weighted least squares after Faucher & Tyson
//! (method 2) and the Good Linear Unbiased Estimator of EN 12603 (method 3).
//! Confidence bands, 5 % fractiles and their bounds, and goodness of fit.

pub mod dist;
pub mod en12603;
pub mod fit;
pub mod gof;
pub mod hypothesis;
pub mod ks;
pub mod random;

pub use fit::{fit_lognormal, fit_weibull, Distribution, Fit, LognormalMethod, Settings, Sides,
              PlottingPositions, WeibullMethod};
