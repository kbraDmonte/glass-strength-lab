"""Synthetic Excel files + expected values for phase 2 (reading, filter
columns, series selection, test-area conversion).

Writes tests/data/*.xlsx (synthetic, no project data) and
tests/golden/phase2.json, using the OLD Python viewer as black-box oracle:

    python tools/make_golden_phase2.py "<path to old project>"
"""
import importlib.util, io, json, os, sys
import numpy as np
import openpyxl

ALT = sys.argv[1]
HIER = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HIER, "..", "tests", "data")
OUT = os.path.join(HIER, "..", "tests", "golden", "phase2.json")
os.makedirs(DATA, exist_ok=True)
sys.argv = ["oracle"]
os.chdir(ALT)
spec = importlib.util.spec_from_file_location("old_viewer", os.path.join(ALT, "Weibull_Viewer.py"))
wv = importlib.util.module_from_spec(spec); spec.loader.exec_module(wv)
P = wv.PROG

rng = np.random.default_rng(7)

def speichern(name, blaetter):
    wb = openpyxl.Workbook()
    wb.remove(wb.active)
    for titel, zeilen in blaetter:
        ws = wb.create_sheet(titel)
        for z in zeilen:
            ws.append(z)
    pfad = os.path.join(DATA, name)
    wb.save(pfad)
    return pfad

# --- file A: sheet "Dataset" after a decoy sheet --------------------------------
kopf_a = ["Measurement number", "ISO number", "Source", "Damage", "Ring size", "Position", "Air/Tin",
          "Sample thickness (in mm)", "radial fracture stress (in N/mm²)", "Remark "]
ringe = ["40:80", "40 / 80", "600 : 800", "18/90", "55/145", None, "weird", "60:120"]
quellen = ["Lab A", "Lab B", "lab a ", "Lab C"]
zeilen_a = [kopf_a]
for i in range(420):
    ring = ringe[i % len(ringe)] if i % 11 else ringe[(i // 7) % len(ringe)]
    basis = {"40:80": 110, "40 / 80": 110, "600 : 800": 45, "18/90": 160, "55/145": 95, "60:120": 100}.get(ring, 90)
    wert = float(np.round(basis * rng.weibull(5.5) + 5, 3))
    if i % 37 == 0: wert = "135,4"
    if i % 53 == 0: wert = None
    if i % 61 == 0: wert = 0
    if i % 67 == 0: wert = -4.2
    erste = i + 1
    if i % 41 == 0: erste = "hide"
    if i % 43 == 0: erste = None
    if i % 47 == 0: erste = "Ausblenden"
    zeilen_a.append([erste, f"P-{1 + i % 12}", quellen[i % 4], ["New", "aged", None][i % 3], ring,
                     1 + i % 4, ["Air", "Tin"][i % 2], round(3.7 + rng.random() * 0.3, 4), wert,
                     "" if i % 5 else "note"])
deko = [["just", "a", "sheet"], [1, 2, 3]]
pfad_a = speichern("synth_a.xlsx", [("Notes", deko), ("Dataset", zeilen_a)])

# --- file B: German sheet name and partly different columns ----------------------
kopf_b = ["Messwertnummer", "ISO number", "Bruchspannung (MPa)", "Ring size", "Extra"]
zeilen_b = [kopf_b]
for i in range(60):
    zeilen_b.append([i + 1, f"Q-{1 + i % 3}", float(np.round(80 * rng.weibull(6) + 10, 3)),
                     ["40:80", "600/800"][i % 2], ["x", "y"][i % 2]])
pfad_b = speichern("synth_b.xlsx", [("Datensatz", zeilen_b)])

# --- broken files ----------------------------------------------------------------
open(os.path.join(DATA, "bad_not_excel.xlsx"), "wb").write(b"this is not an excel file")
speichern("bad_no_sheet.xlsx", [("Foo", [["a", "b"], [1, 2]])])
speichern("bad_no_column.xlsx", [("Dataset", [["ISO number", "Load"], ["P-1", 12.0]])])
speichern("bad_no_values.xlsx", [("Dataset", [["ISO number", "fracture stress"], ["P-1", 0], ["P-2", -3]])])

def lesen(name):
    roh = open(os.path.join(DATA, name), "rb").read()
    try:
        return wv.load_file(roh, name)
    except wv.DataError as err:
        return {"error": err.code, "details": {k: v for k, v in err.details.items()}}

fehler = {n: lesen(n) for n in ("bad_not_excel.xlsx", "bad_no_sheet.xlsx", "bad_no_column.xlsx", "bad_no_values.xlsx")}
fa, fb = lesen("synth_a.xlsx"), lesen("synth_b.xlsx")

def antwort(daten):
    a = wv.data_response(daten)
    return {k: a[k] for k in ("dateien", "spalten", "zeilen", "zielflaechen", "ring_spalte")}

ergebnis = {"errors": fehler, "datasets": {}}
for name, dateien in (("a", [fa]), ("ab", [fa, fb])):
    daten = wv.build_data(dateien)
    sch = {c["name"]: {w["anzeige"]: w["schluessel"] for w in c["werte"]} for c in daten["spalten"]}
    serien = [
        {"name": "all", "filter": {}},
        {"name": "lab a 40", "filter": {"Source": ["lab a"], "Ring size": ["40:80"]}},
        {"name": "two rings", "filter": {"Ring size": ["40 : 80", "600 : 800"]}},
        {"name": "unreadable ring", "filter": {"Ring size": ["weird"]}},
        {"name": "ring 18/90 old spelling", "filter": {"Ring size": ["18/90"], "Air/Tin": ["air"]}},
        {"name": "union", "vereinigung": [{"Source": ["lab b"]}, {"Position": ["1"]}, {"Source": ["lab b"]}]},
        {"name": "non-filter column", "filter": {"Sample thickness (in mm)": ["3.8"]}},
        {"name": "few", "filter": {"Ring size": ["55 : 145"], "Position": ["2"], "Air/Tin": ["tin"]}},
    ]
    faelle = []
    for s in serien:
        for ziel in (0.0, 240000.0, float(np.pi * 9 ** 2), 1256.6370614359173):
            for methode, pos in (("m1_ls", "hazen"), ("m3_glue", "median"), ("m2_wls", "mean")):
                P.PLOTPOSITIONEN, P.KONFIDENZ_SEITEN, P.WEIBULL_METHODE = pos, "einseitig", methode
                w = wv.series_values(daten, s, ziel, methode)
                faelle.append({"series": s, "target_mm2": ziel, "method": methode, "positions": pos,
                               "n": len(w["zeilen"]),
                               "sigma": [float(v) for v in w["sigma"]],
                               "sigma_orig": [float(v) for v in w["sigma_orig"]],
                               "factors": [float(v) for v in w["faktoren"]],
                               "row_keys": [z["s"] for z in w["zeilen"]],
                               "conversion": w["umrechnung"], "hints": w["hinweise"]})
    ergebnis["datasets"][name] = {"files": [d["datei"] for d in dateien], "response": antwort(daten), "cases": faelle}

json.dump(ergebnis, open(OUT, "w", encoding="utf-8"), ensure_ascii=False, indent=0, default=float)
print("written", OUT, {k: len(v["cases"]) for k, v in ergebnis["datasets"].items()}, "errors:", fehler)
