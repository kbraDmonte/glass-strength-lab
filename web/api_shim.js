// Answers the page's requests to /api/... in the browser, with the
// WebAssembly core (crates/viewer-wasm), instead of a local server. The page
// itself is unchanged: it still calls fetch("/api/...") with the same JSON.
//
// Kept in this browser only:
//   - the loaded Excel files (IndexedDB, so they survive a reload),
//   - the saved views (localStorage).
// Nothing is sent anywhere.
"use strict";
(() => {
  const EXAMPLE = "example_data.xlsx";              // synthetic, in this folder
  const VIEWS_KEY = "weibull_viewer_ansichten";
  const DB_NAME = "weibull_viewer", STORE = "dateien";

  const realFetch = window.fetch.bind(window);
  const files = new Map();                          // name -> bytes (as loaded in the core)
  let viewer = null;

  // --- IndexedDB: the list of loaded files (name + bytes, in order) --------
  function db() {
    return new Promise((ok, fail) => {
      const r = indexedDB.open(DB_NAME, 1);
      r.onupgradeneeded = () => r.result.createObjectStore(STORE);
      r.onsuccess = () => ok(r.result);
      r.onerror = () => fail(r.error);
    });
  }
  async function storedFiles() {
    try {
      const d = await db();
      return await new Promise(ok => {
        const r = d.transaction(STORE).objectStore(STORE).get("liste");
        r.onsuccess = () => ok(r.result);
        r.onerror = () => ok(undefined);
      });
    } catch (e) { return undefined; }
  }
  async function storeFiles() {
    const names = JSON.parse(viewer.dateien());
    for (const n of [...files.keys()]) if (!names.includes(n)) files.delete(n);
    try {
      const d = await db();
      d.transaction(STORE, "readwrite").objectStore(STORE)
        .put(names.map(n => ({name: n, bytes: files.get(n)})), "liste");
    } catch (e) { /* private window etc.: then only for this visit */ }
  }

  function views() {
    try { return JSON.parse(localStorage.getItem(VIEWS_KEY) || "{}") || {}; } catch (e) { return {}; }
  }
  function storeViews(v) {
    try { localStorage.setItem(VIEWS_KEY, JSON.stringify(v)); } catch (e) {}
  }

  // returns "" or the error JSON of the core
  function load(bytes, name, mode) {
    const err = viewer.hochladen(bytes, name, mode);
    if (!err) files.set(name, bytes);
    return err;
  }
  async function loadExample() {
    const r = await realFetch(EXAMPLE);
    if (!r.ok) throw new Error(r.status);
    return load(new Uint8Array(await r.arrayBuffer()), EXAMPLE, "ersetzen");
  }

  const ready = (async () => {
    const wasm = await import("./pkg/viewer_wasm.js");
    await wasm.default();
    viewer = new wasm.Viewer();
    const list = await storedFiles();
    if (Array.isArray(list) && list.length) {
      list.forEach((f, i) => load(f.bytes, f.name, i ? "hinzufuegen" : "ersetzen"));
    } else if (list === undefined) {
      // first visit: show the example data
      try { await loadExample(); await storeFiles(); } catch (e) {}
    }
  })();

  const daten = () => viewer.daten(EXAMPLE, true, JSON.stringify(views()));

  async function answer(path, init) {
    await ready;
    const body = init && init.body;
    switch (path) {
      case "/api/daten":
        return daten();
      case "/api/hochladen": {
        const h = new Headers(init.headers);
        const bytes = new Uint8Array(await body.arrayBuffer());
        const err = load(bytes, decodeURIComponent(h.get("X-Dateiname") || "upload.xlsx"), h.get("X-Modus") || "ersetzen");
        if (err) return err;
        await storeFiles();
        return daten();
      }
      case "/api/datei_entfernen":
        viewer.entfernen(JSON.parse(body).datei);
        await storeFiles();
        return daten();
      case "/api/standard_laden": {
        let err;
        try { err = await loadExample(); } catch (e) {
          return JSON.stringify({fehler_code: "standard_fehlt", details: {datei: EXAMPLE}});
        }
        if (err) return err;
        await storeFiles();
        return daten();
      }
      case "/api/ansicht_speichern": {
        const r = JSON.parse(body), v = views();
        v[String(r.name).trim()] = r.ansicht;
        storeViews(v);
        return JSON.stringify({ok: true, ansichten: v});
      }
      case "/api/ansicht_loeschen": {
        const r = JSON.parse(body), v = views();
        delete v[String(r.name)];
        storeViews(v);
        return JSON.stringify({ok: true, ansichten: v});
      }
      default:
        return viewer.anfrage(path, body || "{}");
    }
  }

  window.fetch = async (input, init) => {
    // (input may also be a URL or Request object, e.g. when the WASM file is loaded)
    const url = input instanceof Request ? input.url : String(input);
    if (!url.startsWith("/api/")) return realFetch(input, init);
    let text, status = 200;
    try {
      text = await answer(url, init);
      if (text.length < 4000 && text.includes('"fehler_code"')) status = 400;
    } catch (e) {
      // e.g. a panic in the core ("unreachable"): report like the old server
      text = JSON.stringify({fehler: `${e.name}: ${e.message}`});
      status = 500;
      console.error(e);
    }
    return new Response(text, {status, headers: {"Content-Type": "application/json; charset=utf-8"}});
  };
})();
