// Smoke test for the calculator bundle (#54): loads the WASM build, runs the
// base case and checks every provider's total against the CLI's output in
// docs/cost/output/totals.csv. Run from the repository root:
//
//   node web/calculator/smoke.mjs
import { readFileSync } from "node:fs";
import { initSync, calculate, defaults } from "./pkg/satchel_calc.js";
import { technicalTotals, visibleProviders } from "./view-model.js";

const here = new URL(".", import.meta.url);
initSync({ module: readFileSync(new URL("pkg/satchel_calc_bg.wasm", here)) });

const page = readFileSync(new URL("index.html", here), "utf8");
for (const asset of ["app.js", "style.css"]) {
  if (!page.includes(asset)) throw new Error(`index.html doesn't load ${asset}`);
}

const d = JSON.parse(defaults());
const response = JSON.parse(calculate(JSON.stringify(d.request)));

const csv = readFileSync(new URL("../../docs/cost/output/totals.csv", here), "utf8");
const cli = new Map();
for (const line of csv.trim().split("\n").slice(1)) {
  const [scenario, key] = line.split(",");
  const total = Number(line.slice(line.lastIndexOf(",") + 1));
  if (scenario === "base") cli.set(key, total);
}
if (cli.size === 0) throw new Error("no base rows in totals.csv");

let failed = 0;
for (const p of response.providers) {
  const want = cli.get(p.key);
  const ok = want !== undefined && Math.abs(p.total_usd - want) <= 0.01;
  console.log(`${ok ? "ok  " : "FAIL"} ${p.key}: page ${p.total_usd.toFixed(2)}, CLI ${want}`);
  if (!ok) failed += 1;
}
if (response.providers.length !== cli.size) {
  console.log(`FAIL page has ${response.providers.length} providers, CLI ${cli.size}`);
  failed += 1;
}

// The page deliberately compares cloud storage with Magic Pocket-style
// on-prem storage. The CLI also retains an EC2 model, which is not a Satchel
// deployment option and must never leak into the web presentation.
for (const provider of visibleProviders(response)) {
  if (provider.key === "satchel_ec2") throw new Error("EC2 is visible on the page");
}
const technical = technicalTotals(response);
for (const provider of technical) {
  const want = cli.get(provider.key);
  if (want === undefined || Math.abs(provider.total_usd - want) > 0.01) {
    throw new Error(`technical total differs from CLI for ${provider.key}`);
  }
}

const sensitivity = structuredClone(d.request);
sensitivity.workload.start_bytes *= 10;
const sensitivityResponse = JSON.parse(calculate(JSON.stringify(sensitivity)));
for (const provider of technicalTotals(sensitivityResponse)) {
  const cliProvider = JSON.parse(calculate(JSON.stringify(sensitivity))).providers.find((p) => p.key === provider.key);
  if (!cliProvider || Math.abs(provider.total_usd - cliProvider.total_usd) > 0.01) {
    throw new Error(`technical sensitivity total differs for ${provider.key}`);
  }
}
if (failed) process.exit(1);
console.log("calculator and technical mode match the CLI base case and sensitivity case");
