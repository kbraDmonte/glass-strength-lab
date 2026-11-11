"""Expected values for phase 3 (combine two datasets, Kruskal-Wallis + Dunn)
on the synthetic files of phase 2, using the OLD Python viewer as black-box
oracle:

    python tools/make_golden_phase3.py "<path to old project>"
"""
import importlib.util, json, os, sys

ALT = sys.argv[1]
HIER = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HIER, "..", "tests", "data")
OUT = os.path.join(HIER, "..", "tests", "golden", "phase3.json")
sys.argv = ["oracle"]
os.chdir(ALT)
spec = importlib.util.spec_from_file_location("old_viewer", os.path.join(ALT, "Weibull_Viewer.py"))
wv = importlib.util.module_from_spec(spec); spec.loader.exec_module(wv)

def lesen(name):
    return wv.load_file(open(os.path.join(DATA, name), "rb").read(), name)

EINST = [
    {"methode": "m1_ls", "plotpositionen": "hazen", "konfidenz_seiten": "einseitig", "lognormal_methode": "regression"},
    {"methode": "m2_wls", "plotpositionen": "mean", "konfidenz_seiten": "zweiseitig", "lognormal_methode": "moment"},
    {"methode": "m3_glue", "plotpositionen": "median", "konfidenz_seiten": "einseitig", "lognormal_methode": "moment"},
]
S = {
    "r40": {"name": "Ring 40", "filter": {"Ring size": ["40 : 80"]}},
    "r40_lab_a": {"name": "Lab A 40", "filter": {"Ring size": ["40:80"], "Source": ["lab a"]}},
    "r40_lab_b": {"name": "Lab B 40", "filter": {"Ring size": ["40 : 80"], "Source": ["lab b"]}},
    "r600": {"name": "Ring 600", "filter": {"Ring size": ["600 : 800"]}},
    "r18": {"name": "Ring 18", "filter": {"Ring size": ["18 : 90"]}},
    "mixed": {"name": "Mixed", "filter": {"Ring size": ["40 : 80", "60 : 120"]}},
    "few": {"name": "Few", "filter": {"Ring size": ["55 : 145"], "Position": ["2"], "Air/Tin": ["tin"]}},
    "empty": {"name": "Empty", "filter": {"Source": ["nobody"]}},
    "union": {"name": "Union", "vereinigung": [{"Ring size": ["40 : 80"]}, {"Source": ["lab b"], "Ring size": ["40 : 80"]}]},
    "aged": {"name": "Aged", "filter": {"Damage": ["aged"]}},
}
KOMBI = [("r40_lab_a", "r40_lab_b", 0), ("r40", "r600", 0), ("r40", "r600", 240000.0), ("r18", "r600", 254.46900494077323),
         ("mixed", "r40", 1256.6370614359173), ("r40", "few", 0), ("r40", "empty", 0), ("union", "r40", 0),
         ("aged", "r40", 0)]
GV = [
    {"modus": "serien", "serien": ["r40_lab_a", "r40_lab_b", "r40"]},
    {"modus": "serien", "serien": ["r40", "r600"]},
    {"modus": "serien", "serien": ["r40", "r600", "r18"], "zielflaeche_mm2": 240000.0},
    {"modus": "serien", "serien": ["r40", "empty", "few"]},
    {"modus": "serien", "serien": ["r40"]},
    {"modus": "spalte", "serie": "r40", "spalte": "ISO number"},
    {"modus": "spalte", "serie": "r40", "spalte": "Source", "korrektur": "bonferroni"},
    {"modus": "spalte", "serie": "aged", "spalte": "Ring size"},
    {"modus": "spalte", "serie": "aged", "spalte": "Ring size", "zielflaeche_mm2": 2375.8294442772813},
    {"modus": "spalte", "serie": "r40", "spalte": "Position"},
    {"modus": "spalte", "serie": "r40", "spalte": "Not a column"},
    {"modus": "spalte", "serie": "few", "spalte": "ISO number"},
]

ergebnis = {}
for name, dateien in (("a", ["synth_a.xlsx"]), ("ab", ["synth_a.xlsx", "synth_b.xlsx"])):
    wv.DATA = wv.build_data([lesen(d) for d in dateien])
    faelle = []
    for e in EINST:
        for a, b, ziel in KOMBI:
            req = dict(e, serie_a=S[a], serie_b=S[b], zielflaeche_mm2=ziel)
            faelle.append({"endpoint": "kombinieren", "request": req, "response": wv._nan_to_none(wv.combine(req))})
        for g in GV:
            req = dict(e, **{k: v for k, v in g.items() if k not in ("serien", "serie")})
            if "serien" in g:
                req["serien"] = [dict(S[s], index=i) for i, s in enumerate(g["serien"])]
            else:
                req["serie"] = S[g["serie"]]
            faelle.append({"endpoint": "gruppenvergleich", "request": req,
                           "response": wv._nan_to_none(wv.group_comparison(req))})
    ergebnis[name] = {"files": dateien, "cases": faelle}

json.dump(ergebnis, open(OUT, "w", encoding="utf-8"), ensure_ascii=False, indent=0, default=float)
print("written", OUT, {k: len(v["cases"]) for k, v in ergebnis.items()})
