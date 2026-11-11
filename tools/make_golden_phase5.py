"""Expected values for phase 5 (main calculation for the plots, start-up data)
on the synthetic files, using the OLD Python viewer as black-box oracle:

    python tools/make_golden_phase5.py "<path to old project>"
"""
import importlib.util, json, os, sys

ALT = sys.argv[1]
HIER = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HIER, "..", "tests", "data")
OUT = os.path.join(HIER, "..", "tests", "golden", "phase5.json")
sys.argv = ["oracle"]
os.chdir(ALT)
spec = importlib.util.spec_from_file_location("old_viewer", os.path.join(ALT, "Weibull_Viewer.py"))
wv = importlib.util.module_from_spec(spec); spec.loader.exec_module(wv)

def lesen(name):
    return wv.load_file(open(os.path.join(DATA, name), "rb").read(), name)

wv.DATA = wv.build_data([lesen("synth_a.xlsx"), lesen("synth_b.xlsx")])
S = [{"name": "Ring 40", "farbe": "#2a78d6", "filter": {"Ring size": ["40 : 80"]}},
     {"name": " ", "farbe": "#eb6834", "filter": {"Ring size": ["600 : 800"], "Source": ["lab a"]}},
     {"name": "Empty", "farbe": "#1baf7a", "filter": {"Source": ["nobody"]}},
     {"name": "Union", "vereinigung": [{"Ring size": ["18 : 90"]}, {"Ring size": ["60 : 120"]}]},
     {"name": "Mixed", "filter": {"Ring size": ["40 : 80", "60 : 120", "55 : 145"]}}]
REQ = []
for m, pos, sides, ln in (("m1_ls", "hazen", "einseitig", "regression"), ("m2_wls", "mean", "zweiseitig", "moment"),
                          ("m3_glue", "median", "einseitig", "moment"), ("xx", "yy", "zz", "ww")):
    for ziel in (0, 240000.0, 254.46900494077323):
        REQ.append({"serien": S, "methode": m, "plotpositionen": pos, "konfidenz_seiten": sides,
                    "lognormal_methode": ln, "zielflaeche_mm2": ziel, "hover_spalten": ["ISO number", "Position", "Nope"]})
REQ.append({"serien": [S[2]], "methode": "m1_ls"})
faelle = [{"request": r, "response": wv._nan_to_none(wv.calculate(r))} for r in REQ]
start = wv.data_response(wv.DATA)
statisch = {k: start[k] for k in ("palette", "methoden", "plotpositionen", "seiten", "lognormal_methoden", "max_upload_mb")}
konst = {"P_UNTEN": wv.PROG.P_UNTEN, "P_OBEN": wv.PROG.P_OBEN, "P_TICKS": list(map(float, wv.PROG.P_TICKS)),
         "SIGMA_TICKS": wv.SIGMA_TICKS}
def neu(x):
    """The new page labels the moment estimate without the old suffix."""
    if isinstance(x, dict): return {k: neu(v) for k, v in x.items()}
    if isinstance(x, list): return [neu(v) for v in x]
    if isinstance(x, str) and x.startswith("Momentenschätzung ("): return "Momentenschätzung"
    return x

faelle, statisch = neu(faelle), neu(statisch)
json.dump({"cases": faelle, "static": statisch, "constants": konst}, open(OUT, "w", encoding="utf-8"),
          ensure_ascii=False, indent=0, default=float)
print("written", OUT, len(faelle), json.dumps(statisch, ensure_ascii=False)[:600], konst)
r = faelle[0]["response"]["achsen"]
print("p_ticks:", r["p_ticks"], "y_grenzen:", r["y_grenzen"])
