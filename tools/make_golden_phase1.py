"""Expected values for phase 1 (fits, bands, fractiles, goodness of fit).

Uses the OLD Python viewer only as a black-box oracle: synthetic samples go in,
its results are written to tests/golden/phase1.json. Needs the old project
folder (with Weibull_Viewer.py) as argument; it is not
needed to build or test the Rust code.

    python tools/make_golden_phase1.py "<path to old project>"
"""
import importlib.util, json, os, sys
import numpy as np

ALT = sys.argv[1]
HIER = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HIER, "..", "tests", "golden", "phase1.json")
sys.argv = ["oracle"]
os.chdir(ALT)
spec = importlib.util.spec_from_file_location("old_viewer", os.path.join(ALT, "Weibull_Viewer.py"))
wv = importlib.util.module_from_spec(spec); spec.loader.exec_module(wv)
P = wv.PROG

rng = np.random.default_rng(20261111)
proben = []
for n in (3, 4, 5, 7, 10, 13, 20, 32, 50, 60, 61, 100, 300):
    proben.append(("weibull", n, np.round(120 * rng.weibull(5.0, n) + 10, 3)))
    proben.append(("lognormal", n, np.round(np.exp(rng.normal(4.6, 0.3, n)), 3)))
# ties and a very steep sample
proben.append(("ties", 12, np.array([50, 50, 60, 60, 60, 70, 80, 80, 90, 100, 100, 120], float)))
proben.append(("steep", 30, np.round(100 * rng.weibull(40.0, 30), 3)))

FELDER_W = ["n", "lambda", "beta", "R2", "min", "max", "mean", "std", "frak5", "frak5_95", "frak5_ob",
            "slope", "intercept", "x0"]
FELDER_L = ["n", "mu", "sd", "var", "R2", "min", "max", "mean", "std", "frak5", "frak5_95", "frak5_ob",
            "slope", "intercept", "x0"]

def zahl(v):
    v = float(v)
    return v if np.isfinite(v) else None

faelle = []
for art, n, werte in proben:
    s = np.sort(werte)
    for methode in ("m1_ls", "m2_wls", "m3_glue"):
        for pos in ("hazen", "mean", "median"):
            for seiten in ("einseitig", "zweiseitig"):
                for ln_methode in ("moment", "regression"):
                    P.PLOTPOSITIONEN, P.KONFIDENZ_SEITEN, P.WEIBULL_METHODE = pos, seiten, methode
                    fall = {"sample": art, "n": int(n), "values": [float(x) for x in werte],
                            "method": methode, "positions": pos, "sides": seiten, "lognormal_method": ln_methode}
                    try:
                        fits = wv.evaluate(s, methode, ln_methode)
                    except SystemExit as err:
                        fall["error"] = str(err)
                        faelle.append(fall); continue
                    x_grid = np.linspace(np.log(s.min()) - 0.5, np.log(s.max()) + 0.3, 7)
                    for dist, felder in (("weibull", FELDER_W), ("lognormal", FELDER_L)):
                        res = fits[dist]
                        e = {k: zahl(res[k]) for k in felder if k in res}
                        e.update({k: zahl(res[k]) for k in ("A2", "D", "p_AD", "std_fit")})
                        e["P"] = [float(p) for p in res["P"]]
                        e["y"] = [float(y) for y in res["y"]]
                        e["band_x"] = [float(x) for x in x_grid]
                        if True:
                            e["band_lower"] = [zahl(v) for v in P.band_y(res, x_grid, "unten")]
                            e["band_upper"] = [zahl(v) for v in P.band_y(res, x_grid, "oben")]
                        fall[dist] = e
                    faelle.append(fall)

os.makedirs(os.path.dirname(OUT), exist_ok=True)
json.dump({"source": "old Python viewer (oracle), synthetic samples", "cases": faelle},
          open(OUT, "w", encoding="utf-8"), indent=0)
print(len(faelle), "cases ->", OUT)
print("errors:", sum(1 for f in faelle if "error" in f))
