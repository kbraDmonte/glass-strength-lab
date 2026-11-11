# Glass Strength Lab

Interactive evaluation of glass fracture strength from coaxial double-ring tests
(DIN EN 1288-2 / -5). Developed by [kbraDmonte](https://github.com/kbraDmonte), student assistant (HiWi) at
the [Institute of Structural Mechanics and Design (ISMD)](https://www.ismd.tu-darmstadt.de),
TU Darmstadt.

**Open it:** <https://kbradmonte.github.io/glass-strength-lab/>

Pick test series with filters. The viewer fits **2-parameter Weibull** and
**log-normal** distributions, shows them on probability paper and reports the
characteristic strength: the 5 % fractile σk;0.05 and its lower 95 %
confidence bound σk;0.05;0.95. It can also convert between test areas,
compare and pool series, and validate the fits out of sample.

**Everything runs in your browser.** The calculation core is written in Rust
and compiled to WebAssembly. Your Excel files are read locally and are
**never uploaded anywhere**. They are kept in the browser's own storage
(IndexedDB), so they are still there after a reload.

> No measurement data are in this repository. The example file
> `web/example_data.xlsx` and all test files are synthetic.

---

## Features

| Area | What it does |
|---|---|
| **Test series** | Build series from filters (AND between columns, OR within a column). Split a series by any column, e.g. one series per pane. |
| **Fits** | Weibull by least squares (method 1), weighted least squares (method 2, Faucher & Tyson) or GLUE / EN 12603 (method 3). Log-normal by method of moments or by regression. Plotting positions Hazen, mean rank or median rank. One- or two-sided 95 % confidence. |
| **Preset** | The button *TU Delft Methode 1* sets method 1, Hazen, one-sided and log-normal by regression, as in TU Delft's open-GLASSlab. |
| **Plots** | Weibull and/or log-normal probability paper, overlaid or one per series. Fitted line with its equation, confidence band, 5 % fractiles. |
| **Results** | m and β (or μ and s), R², A², min/max/mean, both fractiles, scatter of the data and of the fitted distribution. CSV export. |
| **Test-area conversion** | Size effect σ_A1 = σ_A0 · (A0/A1)^(1/m), applied per ring size. The target is a standard set-up or any ring size in the data. |
| **Combine datasets** | Anderson–Darling test per series, Pearson correlation of Q-Q pairs, two-sample Anderson–Darling test and a Q-Q plot. Creates the pooled series. |
| **Group comparison** | Kruskal–Wallis test across all groups, Dunn post-hoc test (Holm or Bonferroni), compact letter display, p-value matrix. |
| **Validation** | k-fold cross-validation, leave-one-out, repeated hold-out, bootstrap, Anderson–Darling with parametric bootstrap, leave-one-group-out. |
| **Other** | Several Excel files at once, saved views (also as .json files), German/English, light/dark theme. |

## Data format

The viewer reads one worksheet per `.xlsx` file. Any table that follows these
rules works.

### Required

| Rule | Details |
|---|---|
| **File type** | `.xlsx` (Excel 2007 or newer). `.xls` and `.csv` are not supported. Maximum file size: 50 MB. |
| **Sheet name** | `Dataset` or `Datensatz`, exact name. If both exist, `Dataset` is used. |
| **Header row** | The **first row** holds the column names. Data start in row 2. |
| **Strength column** | One column whose name contains **`fracture stress`** or **`Bruchspannung`** (case does not matter), e.g. `radial fracture stress (in N/mm²)`. Values are in N/mm² (= MPa). If several columns match, the first one is used. |
| **Values** | Numbers, or text with a decimal comma or point (`135,4` and `135.4` both work). Rows with an empty, non-numeric, zero or negative strength are skipped. |

### Optional columns

Every other column automatically becomes a **filter** if it has 2–200
different text values or 2–40 different numbers. Columns with more distinct
values, such as thickness or force, are not offered as filters, but they can
still be shown in the hover details. Column names are free; typical ones:

| Column (example name) | Used for |
|---|---|
| `Ring size` / `Ringgröße` | **Test-area conversion.** Load-ring and support-ring diameters in mm, e.g. `40:80`, `55 / 145` or `600 : 800` (`:` or `/`, spaces optional). Shown as `40 : 80`. Without this column, conversion is not available. |
| `ISO number`, `Position`, `Air/Tin` | Identify panes and specimen positions: filters, split by pane, leave-one-group-out validation. |
| `Source`, `Damage`, `Manufacturer`, … | Any grouping you want to filter or compare by. |
| `Measurement number`, `Sample number`, `Messwertnummer`, `Probennummer` | Running numbers. These are never offered as filters. |

A row is ignored if its **first cell is empty**, or if the first cell contains
`hide` or `ausblenden`. This excludes specimens without deleting them.

**Several files:** *Add file …* loads another file next to the ones already
loaded. Columns are matched **by name**. A column `Excel-Datei` shows which
file each specimen comes from. Loading identical content twice is refused, so
no specimen is counted twice.

---

## Running it

The published page needs no installation. To run it locally, serve the
`web/` folder over HTTP after building (see below). Opening `index.html` as a
file does not work, because browsers do not load WebAssembly from `file://`.

```bash
python -m http.server 8000 --directory web    # then open http://127.0.0.1:8000
```

## Building

Requirements: Rust (stable), the target `wasm32-unknown-unknown`, and
[`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

```bash
rustup target add wasm32-unknown-unknown
wasm-pack build crates/viewer-wasm --release --target web --out-dir ../../web/pkg --no-typescript
cargo test --release
```

Pushing to `main` builds, tests and publishes the page via GitHub Pages
(`.github/workflows/pages.yml`). In the repository settings, set
*Pages → Source* to *GitHub Actions*.

## Layout

```
crates/glass-stats   statistics: plotting positions, Weibull methods 1–3,
                     log-normal, confidence bands, fractiles, goodness of fit,
                     hypothesis tests, random numbers
crates/viewer-core   Excel reading, series, test-area conversion, combine,
                     group comparison, validation, the page's JSON requests
crates/viewer-wasm   WebAssembly wrapper (wasm-bindgen)
web/                 the page, api_shim.js (answers /api/... in the browser),
                     Plotly, example data
tests/               synthetic Excel files and expected results (golden/)
tools/               scripts that generated the expected results (see below)
```

## How the results were verified

This is an independent rewrite of an earlier Python prototype of the viewer
(research project AP3 Re-Use-Glas, Hochschule München); no code was taken from
it. The expected values in `tests/golden/` were produced by running that
prototype as a black box on synthetic data. `cargo test --release` checks:

- **Fits and bands:** 1008 cases, 162 288 values, all methods and settings,
  within 1e-7 relative.
- **Data handling and conversion:** 192 series selections and 4 faulty files,
  exact.
- **Comparisons:** combine and Kruskal–Wallis/Dunn, 126 requests, within 1e-7.
- **Validation:** deterministic parts (leave-one-out, leave-one-group-out)
  are exact. The randomised checks use a different random number generator,
  so they are compared statistically.
- **Main calculation for the plots:** 13 requests, all settings, with and
  without test-area conversion.

In addition, the page was driven through the same 31 interface steps in both
versions. The visible text was identical except for the randomised validation
figures.

## Credits and sources

- **Statistical methods 1–3** follow Cupać, J., Louter, C., Nikolov, G.,
  Schuster, M., Datsiou, K. C. (2025). *Open GLASSlab – Interactive Strength
  Analysis Handbook*. TU Delft. <https://oit.tudelft.nl/open-GLASSlab/>
  (source: <https://github.com/TUDelft-books/open-GLASSlab>), licensed
  [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). The methods were
  implemented independently from the descriptions there; no code was copied.
  This project is not affiliated with or endorsed by TU Delft or the
  handbook's authors.
- **Method 3 (GLUE)** follows EN 12603 *Glass in building – Procedures for
  goodness of fit and confidence intervals for Weibull distributed glass
  strength data*. The standard is cited, not reproduced; the κₙ coefficients
  are the published table values.
- **k-sample Anderson–Darling test:** Scholz, F. W., Stephens, M. A. (1987),
  *K-Sample Anderson–Darling Tests*, JASA 82(399). **Dunn test:** Dunn, O. J.
  (1964), Technometrics 6(3). **Compact letter display:** Piepho, H.-P. (2004),
  J. Comput. Graph. Stat. 13(2). **KS distribution:** Marsaglia, G., Tsang,
  W. W., Wang, J. (2003), J. Stat. Softw. 8(18).
- **[Plotly.js](https://github.com/plotly/plotly.js)** 2.35.2 (MIT), bundled
  as `web/plotly.min.js`.
- Rust crates: calamine, statrs, serde_json, regex, sha2, wasm-bindgen, each
  under its own license (MIT and/or Apache-2.0).

See [`NOTICE`](NOTICE).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
