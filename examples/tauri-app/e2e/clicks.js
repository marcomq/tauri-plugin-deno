// Runs inside the app window (see run.mjs): clicks the buttons like a user and checks the output.
// Waits use a MutationObserver, since WebKit throttles timers in hidden windows.
(async () => {
  const { emit } = window.__TAURI__.event;
  const results = [];
  const input = document.querySelector("#input-field");
  const output = document.querySelector("#output-element");

  const waitForOutput = (button, expected) =>
    new Promise((resolve) => {
      const check = () => {
        emit("e2e-progress", { ok: false, results, waiting: button, output: output.textContent });
        if (expected.test(output.textContent)) {
          observer.disconnect();
          resolve();
        }
      };
      const observer = new MutationObserver(check);
      observer.observe(output, { childList: true, characterData: true, subtree: true });
    });

  const click = async (button, text, expected) => {
    input.value = text;
    output.textContent = "";
    const shown = waitForOutput(button, expected);
    document.querySelector(`button[value=${button}]`).click();
    await shown;
    results.push(`${button}: ${output.textContent}`);
  };

  try {
    // main.js attaches its submit handlers after an awaited `listen`, then signals readiness.
    if (!window.appReady) {
      await new Promise((resolve) => window.addEventListener("app-ready", resolve, { once: true }));
    }
    await click("submit_rust", "E2E", /^Hello, E2E! You've been greeted from Rust!$/);
    await click("submit_deno", "E2E", /^Hello, E2E! You've been greeted 1 time from Deno\/JS!$/);
    await click("submit_deno", "E2E", /greeted 2 times/);
    await click("submit_fail", "", /^Caught: .*thrown in Deno/);
    await click("submit_square", "12", /^12² = 144 \(computed in Deno\)$/);
    await click("submit_ping", "hi", /^Deno answered: \{"received":"hi","at":"/);
    await emit("e2e-done", { ok: true, results });
  } catch (error) {
    await emit("e2e-done", { ok: false, results, error: String(error) });
  }
})();
