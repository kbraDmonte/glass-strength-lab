//! Data handling and request logic of Glass Strength Lab.
//!
//! Everything here works on plain Rust values and answers with the same JSON
//! the previous Python server produced, so the browser page can stay as it is.

pub mod area;
pub mod calculate;
pub mod compare;
pub mod data;
pub mod results;
pub mod state;
pub mod validation;
pub mod excel;
pub mod text;

pub use data::{build_data, data_response, select_rows, series_rows, series_values, Data, Row,
               SeriesValues};
pub use excel::{read_excel, Cell, DataError, ExcelFile};
