"""Expected values for phase 4 (validation suite) on the synthetic files,
using the OLD Python viewer as black-box oracle:

    python tools/make_golden_phase4.py "<path to old project>"
"""
import importlib.util, json, os, sys

ALT = sys.argv[1]
HIER = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HIER, "..", "tests", "data")
OUT = os.path.join(HIER, "..", "tests", "golden", "phase4.json")
sys.argv = ["oracle"]
os.chdir(ALT)
spec = importlib.util.spec_from_file_location("old_viewer", os.path.join(ALT, "Weibull_Viewer.py"))
wv = importlib.util.module_from_spec(spec); spec.loader.exec_module(wv)

def lesen(name):
    return wv.load_file(open(os.path.join(DATA, name), "rb").read(), name)

wv.DATA = wv.build_data([lesen("synth_a.xlsx"), lesen("synth_b.xlsx")])
TUD = {"methode": "m1_ls", "plotpositionen": "hazen", "konfidenz_seiten": "einseitig", "lognormal_methode": "regression"}
M3 = {"methode": "m3_glue", "plotpositionen": "median", "konfidenz_seiten": "zweiseitig", "lognormal_methode": "moment"}
REQ = [
    dict(TUD, serie={"name": "Ring 40", "filter": {"Ring size": ["40 : 80"]}}, k=5, gruppen_spalte="ISO number",
         hover_spalten=["ISO number", "Position"]),
    dict(TUD, serie={"name": "Ring 600", "filter": {"Ring size": ["600 : 800"]}}, k=10, gruppen_spalte="Excel-Datei",
         split_anteil=0.8),
    dict(M3, serie={"name": "Lab A", "filter": {"Source": ["lab a"]}}, k=5, gruppen_spalte="", split_anteil=0.5),
    dict(TUD, serie={"name": "All", "filter": {}}, k=5, gruppen_spalte="Source", zielflaeche_mm2=0),
    dict(TUD, serie={"name": "Converted", "filter": {"Ring size": ["40 : 80", "60 : 120"]}}, k=5,
         gruppen_spalte="Ring size", zielflaeche_mm2=240000.0),
    dict(TUD, serie={"name": "Few", "filter": {"Ring size": ["55 : 145"], "Position": ["2"], "Air/Tin": ["tin"]}}, k=5),
    dict(TUD, serie={"name": "One group", "filter": {"Ring size": ["40 : 80"]}}, k=5, gruppen_spalte="Damage",
         split_anteil=0.33),
]
faelle = [{"request": r, "response": wv._nan_to_none(wv.validate(r))} for r in REQ]
json.dump({"files": ["synth_a.xlsx", "synth_b.xlsx"], "cases": faelle}, open(OUT, "w", encoding="utf-8"),
          ensure_ascii=False, indent=0, default=float)
print("written", OUT, [c["response"].get("n", c["response"].get("fehler_code")) for c in faelle])
