// Usage: node e2e/run.mjs <app binary>
// Starts the app with the click script and fails unless all clicks gave the expected output.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const [binary] = process.argv.slice(2);
if (!binary) {
  console.error("usage: node e2e/run.mjs <app binary>");
  process.exit(2);
}
const report = join(mkdtempSync(join(tmpdir(), "deno-e2e-")), "report.json");
const child = spawn(binary, [], {
  stdio: "inherit",
  env: {
    ...process.env,
    TAURI_E2E_SCRIPT: fileURLToPath(new URL("./clicks.js", import.meta.url)),
    TAURI_E2E_REPORT: report,
  },
});
const timer = setTimeout(() => {
  console.error("E2E timeout: no result after 60 s");
  child.kill();
}, 60_000);
child.on("exit", (code) => {
  clearTimeout(timer);
  let result;
  try {
    result = JSON.parse(readFileSync(report, "utf8"));
  } catch {
    console.error(`E2E failed: no report (exit code ${code})`);
    process.exit(1);
  }
  for (const line of result.results) console.log(`  ok  ${line}`);
  if (result.waiting) {
    console.error(`  FAIL ${result.waiting}: unexpected output '${result.output}'`);
  } else if (!result.ok) {
    console.error(`  FAIL ${result.error}`);
  }
  process.exit(result.ok && code === 0 ? 0 : 1);
});
