# Verbesserungen für tauri-plugin-deno

Abgeleitet aus den Arbeiten an `tauri-plugin-python` (Fehlerbehandlung, Tests, CI,
Release, Debugging-Doku). Stand der Analyse: `main` @ `c7c3b59`.

Nicht übertragbar ist das Deployment-Tooling aus dem Python-Plugin (`cli/` mit
`setup`/`doctor`/`sign`, `pyembed`, `docs/deploy-*.md`). V8/Deno ist statisch in das
Binary gelinkt; es gibt keine Laufzeit-Bibliothek, die man bündeln oder separat
signieren müsste. Tauris eigenes Signing reicht.

## Stand 13.11.2026 (Version 0.4.0)

Umgesetzt: Punkte 1–4, 6–9, 11 sowie Teile von 10 (Edition 2024, `lazy_static`/`winapi`
entfernt, Emit-Bridge ohne Mutex). Punkt 5: stdio-Guard eingebaut, auf einem echten
Windows-Release noch **nicht verifiziert** (CI baut das Beispiel auch unter Windows).
Die Deno-Crates wurden entgegen Punkt 0/10 doch aktualisiert: Der alte Stand ließ sich
mit aktuellem serde und unter macOS 27 nicht mehr bauen. Entscheidung: vorerst bei Deno
bleiben, ein anderes Engine-Backend ggf. später als eigenes Plugin.

## 0. Wichtig vorab: Deno wird eingestellt

Am 9. Oktober 2026 hat Deno angekündigt, dass das gesamte Team zu Cloudflare wechselt
([deno.com/blog/cloudflare](https://deno.com/blog/cloudflare)):

- Die Runtime bekommt noch **ein Jahr** lang monatliche Releases mit Bugfixes und
  Sicherheitsupdates (also bis etwa Oktober 2027). Danach endet die Entwicklung. Deno
  bleibt Open Source, ob jemand es weiterführt, ist offen.
- Deno Deploy wird nach etwa sechs Monaten abgeschaltet. JSR läuft weiter.

Folge für dieses Plugin: `deno_core`, `deno_runtime` und `deno_fs` bekommen nur noch
Wartung und danach voraussichtlich nichts mehr. V8 ohne Sicherheitsupdates in einer
Desktop-App einzubetten, ist keine tragfähige Basis.

**Konsequenzen für diese Liste**
- **Lohnt sich weiterhin** (günstig, macht das Plugin bis zum Umstieg stabil):
  Punkte 1, 2, 4, 7 und die Doku in Punkt 11.
- **Nur, wenn das Plugin auf Deno-Basis noch länger gepflegt werden soll:** Punkte
  3, 5, 6, 8, 9.
- **Nicht mehr empfohlen:** das Update der Deno-Crates aus Punkt 10.
- README: einen Hinweis auf das Ende von Deno und die geplante Zukunft des Plugins
  aufnehmen.

**Mögliche Zukunft des Plugins (Entscheidung offen)**
- **Sidecar mit Node oder Bun:** `bun build --compile` bzw. Node-SEA als `externalBin`,
  Kommunikation per stdio (JSON-Zeilen). Langfristig am wenigsten Wartung, voller
  npm-Zugriff, und ein JS-Absturz beendet nicht die App. Nachteile: etwa 50–100 MB
  größere App, zweiter Prozess, das Sidecar muss pro Plattform gebaut und auf macOS
  signiert werden. Die Frontend-API (`callFunction`, `emit`, …) kann gleich bleiben.
- **Eingebettete schlanke Engine (`rquickjs` oder Boa):** ein Prozess, kleine App,
  kein clang/LLVM unter Windows. Es gibt aber keine Deno- oder Node-APIs (Dateien,
  `fetch`, …). Benötigte Funktionen müssten als Rust-Funktionen bereitgestellt werden.
- Vorgehen bei beiden: zuerst die Tests aus Punkt 8 gegen die öffentliche Frontend-API
  schreiben, dann die Engine dahinter austauschen.

---

Reihenfolge = Priorität. Jeder Punkt hat ein Prüfkriterium.

---

## 1. Ein JS-Fehler darf den Deno-Thread nicht beenden (kritisch)

Der gesamte Deno-Code läuft in **einem** Thread (`deno_lib::js_runtime_thread`). Jeder
`panic!` dort beendet ihn. Danach schlägt jeder weitere Aufruf fehl: Das `.unwrap()`
auf `send` in `commands.rs` panickt, bzw. `receiver.await.unwrap_or_default()` liefert
stillschweigend `null`.

Panic-Stellen im Request-Pfad (`src/deno_lib.rs`):

| Stelle | Auslöser |
| ------ | -------- |
| `deno_tokio_loop`: `&global_fns[&req.function_name]` | `callFunction` mit nicht registriertem Namen |
| `get_fn`: `.expect("missing function")`, `.expect("function expected")` | `registerFunction` mit unbekanntem Namen / Nicht-Funktion |
| `deno_exec_js`: `execute_script(...).unwrap()` | jeder Syntaxfehler oder jede Exception in `runCode` |
| `vec_to_v8_vec`: `serde_v8::to_v8(...).unwrap()` | nicht konvertierbares Argument |
| `op_emit` (`deno_ops.rs`): `.expect("cannot send message as emit")` | Emit-Kanal geschlossen |
| `handle_emit`: `app.emit(...).unwrap()` | Emit schlägt fehl |
| `commands.rs` Makro: `.send(...).await.unwrap()` | Deno-Thread bereits tot |

**Umsetzung**
- `get_fn` gibt `Result` zurück; `CallFnRequest` nutzt `global_fns.get(..)` und liefert
  `"Function '<name>' has not been registered yet"`.
- `deno_exec_js` mit `?` statt `unwrap()`.
- In `commands.rs` `send`-Fehler → `Error::String("Deno runtime is not running")`.
- Emit-Fehler loggen statt panicken.

**Prüfung:** Unit-/Integrationstest: `callFunction("doesNotExist")`, danach ein gültiger
`callFunction` → der zweite Aufruf liefert das richtige Ergebnis.

## 2. Fehler als Promise-Rejection statt als String-Ergebnis

Aktuell wird in `deno_tokio_loop` jeder Fehler zu
`Ok(Value::String("error: ..."))`. Das Frontend kann Fehler nicht von einem
regulären String-Rückgabewert unterscheiden; `try/catch` greift nie.

Im Python-Plugin geben die Commands `Err(Error)` zurück, und das Promise im
Frontend wird rejected.

**Umsetzung**
- `JsMsg.responder` von `oneshot::Sender<Value>` auf
  `oneshot::Sender<Result<Value, String>>` (oder `crate::Result<Value>`) umstellen.
- Im Makro `send_receive_result!` das Ergebnis durchreichen; `unwrap_or_default()`
  (verschluckt einen toten Thread als `null`) durch einen Fehler ersetzen.
- Kontext voranstellen wie bei `py_context` im Python-Plugin, z. B.
  `Error calling Deno function 'greetJs': <msg>`,
  `Cannot register 'x': not found in main.js (is it defined and passed to addTauri?)`.
- `v8_to_js` verliert die eigentliche Meldung (`CoreError::Parse(default)` +
  `eprintln!`) → Meldung in den Fehler übernehmen.
- `error.rs`: ungenutzte `From`-Impls für `std::sync::mpsc`/`broadcast` entfernen
  (das Plugin nutzt nur `tokio::mpsc`); `From<tauri::Error>` und
  `From<deno_core::error::CoreError>` ergänzen.

**Prüfung:** Im Beispiel `await callFunction("throwsError", [])` → landet im `catch`;
README-Snippet mit `try/catch` analog zum Python-README.

## 3. Stacktraces: Debug an, Release aus, per Env-Var einschaltbar

Übernahme von `tracebacks_enabled` aus dem Python-Plugin (`src/lib.rs`, Commit 58f9c67):

- Debug-Builds (`cfg!(debug_assertions)`): Fehler enthalten den JS-Stack
  (`CoreError::Js` → `JsError.stack`) und werden mit Präfix
  `[tauri-plugin-deno]` nach stderr geloggt.
- Release-Builds: nur die Meldung (`exception_message`), kein Stack, kein Logging.
  So landen keine Pfade/Quelltextzeilen beim Endnutzer.
- `TAURI_PLUGIN_DENO_TRACEBACK=1|true` schaltet beides im Release ein.

**Prüfung:** Unit-Tests wie `mod traceback` in `tauri-plugin-python/src/unit_tests.rs`
(`env_var_enables_tracebacks`, `without_env_var_only_debug_builds_have_tracebacks`).

## 4. Start-Fehler lesbar melden

Beim Start gibt es mehrere `unwrap()`/`expect()` mit wenig hilfreichen Meldungen:
`execute_script("deno_dist.js", code).unwrap()`,
`run_event_loop(..).expect("Error initializing deno-dist.js")`,
`function_name.as_str().unwrap()`, `register_fn(..).expect(..)`.

**Umsetzung:** wie `startup_failed` im Python-Plugin: eine Funktion, die
`"[tauri-plugin-deno] <Kontext>: <Display-Fehler>"` ausgibt (kein `Debug`-Format) und
dann abbricht. Ggf. mit Hinweisen, z. B. „Is `deno_dist.js` up to date? Run the
bundler before `tauri build`."

Zusätzlich: `println!("exit thread")` → entfernen oder `eprintln!` mit Präfix.

## 5. Windows-Release: verstecktes Konsolenfenster (Hypothese für den offenen Bug)

Das README nennt als offenes Problem: „Deno code isn't called on windows production
binaries". Das Beispiel nutzt
`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`. Im
Python-Plugin war genau das die Ursache (ungültige stdout/stderr-Handles ohne
Konsole, Issues #4/#15/#17).

Vermutung: `console.log` in `main.js` wirft im Release ohne Konsole, und durch das
`unwrap()` in `js_runtime_thread` stirbt der Deno-Thread schon beim Start. Das ist
noch **nicht verifiziert**.

**Vorgehen**
1. Erst Punkt 1 + 4 umsetzen, dann erscheint die echte Fehlermeldung statt eines
   stillen Ausfalls.
2. Windows-Release-Build des Beispiels mit `console.log` in `main.js` starten.
   Bestätigt sich die Vermutung: in `plugin_runtime.js` `console.log/error/...` (oder
   die stdio-Ops) in `try/catch` kapseln, analog zu `PY_STDIO_GUARD`.
3. README-Abschnitt „Windows: hidden console & stdio" ergänzen.

## 6. Timeout pro Aufruf

Ein hängender JS-Aufruf (z. B. ein `fetch` ohne Timeout, endloses `await`) blockiert
den einzigen Deno-Thread und damit alle späteren Aufrufe.

**Umsetzung:** im Command `tokio::time::timeout` um `receiver.await`.
Default 300 s, Override per `TAURI_PLUGIN_DENO_TIMEOUT_SECS` (`0` = aus). Logik wie
`call_timeout` im Python-Plugin (inkl. Unit-Test). Doku: Der Thread bleibt bis zum
Ende des hängenden Aufrufs belegt; nur das Frontend bekommt sofort einen Fehler.

## 7. Bekannte Bugs außerhalb der Fehlerbehandlung

- **`registerFunction` aus dem Frontend ist kaputt:** `guest-js/index.ts` sendet
  `payload: { denoFunctionCall, numberOfArgs }`, Rust erwartet `functionName`
  (`RegisterRequest`, `rename_all = "camelCase"`). → Key auf `functionName`
  umbenennen. Prüfung: Test, der den Command mit dem Frontend-Payload deserialisiert.
- **`numberOfArgs` wird ignoriert:** `register_fn` prüft die Argumentanzahl nicht.
  Entweder via `fn.length` validieren (wie die `inspect.signature`-Prüfung im
  Python-Plugin, nur bei echtem Mismatch ablehnen) oder den Parameter entfernen.
- **`deno_dist.js`-Pfad ist fragil:** `include_str!(concat!(env!("OUT_DIR"),
  "/../../../../../target/deno_dist.js"))` zählt Verzeichnisebenen. Mit
  `--target <triple>` (z. B. Cross-Builds, Universal-macOS) verschiebt sich das um
  eine Ebene. Fehlt die Datei, bricht der Build mit einer kryptischen Meldung ab.
  → In `build.rs` den Pfad auflösen (Env-Var `TAURI_PLUGIN_DENO_DIST` mit Fallback),
  bei fehlender Datei eine klare `panic!`-Meldung mit Hinweis auf den Bundler, und
  `cargo:rerun-if-changed` setzen.
- **README-Beispiel stimmt nicht:** `return str(input)` ist Python, nicht JS;
  `window.document.body.innerText = window.__TAURI__.deno.callFunction(...)` fehlt
  `await`. Außerdem `addTauri(fn)` dokumentieren (in `plugin_runtime.js` vorhanden).

## 8. Tests

Aktuell gibt es keine Tests. Vorbild: `tauri-plugin-python/src/unit_tests.rs`
(reine Logik) und `src/tests.rs` (Commands über `tauri::test::mock_builder`).

- `Cargo.toml`: `[dev-dependencies] tauri = { version = "2", features = ["test"] }`.
- Unit-Tests: Timeout-/Traceback-Parsing, Payload-Deserialisierung (Punkt 7).
- Integrationstests gegen die echte Runtime: run/read/register/call, Fehlerfälle aus
  Punkt 1–2, „Thread lebt nach Fehler weiter".
  Dafür muss `deno_dist.js` im Test austauschbar sein (siehe Pfad-Fix in Punkt 7).
- **Windows-Testbinaries:** `windows-app-manifest.xml` + der MSVC-Block aus
  `tauri-plugin-python/build.rs` übernehmen. Ohne ihn starten Testbinaries mit
  Tauri-Abhängigkeit unter Windows mit `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)`.

## 9. CI und Release

Vorlagen: `tauri-plugin-python/.github/workflows/{test,release}.yml`.

- `test.yml`:
  - `lint`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
  - `test`: Matrix ubuntu-22.04 / windows-latest / macos-latest, `cargo test --all-targets`.
    Windows braucht LLVM/clang (README-Voraussetzung), Linux die üblichen
    webkit2gtk-Pakete.
  - `example-bundle`: Matrix, Bundler + `tauri build` des Beispiels, dann das gebaute
    Binary kurz starten (wie `launch_bundled.mjs` im Python-Beispiel). Damit wird der
    Windows-Release-Bug aus Punkt 5 dauerhaft abgedeckt.
- `release.yml`: Tag-Trigger + `workflow_dispatch`, Publish nach crates.io und npm per
  Trusted Publishing; bereits veröffentlichte Versionen überspringen.
- `Cargo.lock` des Beispiels committen (aktuell in `.gitignore`), damit CI reproduzierbar
  baut.

## 10. Aufräumen und Aktualisierungen (niedrige Priorität)

- Deno-Crates sind alt (`deno_core 0.338`, `deno_runtime 0.201`, …). **Nicht mehr
  empfohlen** (siehe Punkt 0). Nur machen, wenn das Plugin bis zum Ende der
  Deno-Wartung auf Deno bleiben soll, dann alle Crates gemeinsam aktualisieren und
  erst nach den Tests aus Punkt 8.
- `edition = "2024"` / `rust-version` anheben wie im Python-Plugin.
- `lazy_static` → `std::sync::LazyLock`.
- `winapi` ist als Dependency eingetragen, wird aber nirgends genutzt. Prüfen und ggf.
  entfernen.
- `emit_handler_thread` hält `blocking_lock()` auf den Mutex, während er in
  `blocking_recv()` wartet. Den Receiver einmalig herausnehmen (`take`) statt in
  jeder Schleife zu sperren.

## 11. Doku (README)

Analog zum Python-README ergänzen:
- **Debugging:** Fehler kommen als Promise-Rejection, Stack in Debug-Builds, stderr-Präfix,
  `TAURI_PLUGIN_DENO_TRACEBACK`, häufige Fehlermeldungen und ihre Ursachen.
- **„Works in `tauri dev` but not in the production build“-Checkliste:** Bundler vor
  `tauri build` gelaufen? `deno_dist.js` aktuell? Windows-Konsole? Mit Konsole starten,
  um Startfehler zu sehen.
- **Call timeout** und **Windows: hidden console** (Punkte 5–6).
- **Security:** `PermissionsContainer::allow_all` ausdrücklich nennen. Der JS-Code hat
  vollen Datei-/Netz-/Prozesszugriff. Die UI darf nicht über eine Netzwerk-URL
  erreichbar sein, wenn `run-code`/`register-function` freigegeben sind.
