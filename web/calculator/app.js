// Satchel pricing calculator (#54). All the numbers come from the cost engine
// in crates/satchel-cost, compiled to WASM (crates/satchel-calc). This file
// only reads the inputs, keeps them in the URL and draws the results.
import init, { calculate, defaults } from "./pkg/satchel_calc.js";

const $ = (id) => document.getElementById(id);
const GIB = 1073741824;
const PB = 1e15;

let base; // the base-case request from the engine
let prices; // default price tables with sources
let current; // the request on screen
let last; // the latest response
// Series hidden from the chart. Satchel on EC2 costs far more than the rest
// at Dropbox's scale and flattens every other line, so it starts hidden.
const hidden = new Set(["satchel_ec2"]);

// ---------- formatting ----------

function usd(x) {
  if (!Number.isFinite(x)) return "n/a";
  const a = Math.abs(x);
  if (a >= 1e9) return `$${(x / 1e9).toFixed(2)}B`;
  if (a >= 1e6) return `$${(x / 1e6).toFixed(1)}M`;
  if (a >= 1e3) return `$${(x / 1e3).toFixed(1)}k`;
  return `$${x.toFixed(0)}`;
}

function bytes(x) {
  if (!Number.isFinite(x)) return "n/a";
  const units = [["EB", 1e18], ["PB", 1e15], ["TB", 1e12], ["GB", 1e9], ["MB", 1e6]];
  for (const [u, v] of units) if (Math.abs(x) >= v) return `${(x / v).toPrecision(3)} ${u}`;
  return `${x.toFixed(0)} B`;
}

function count(x) {
  if (!Number.isFinite(x)) return "n/a";
  if (x >= 1e12) return `${(x / 1e12).toPrecision(3)} trillion`;
  if (x >= 1e9) return `${(x / 1e9).toPrecision(3)} billion`;
  if (x >= 1e6) return `${(x / 1e6).toPrecision(3)} million`;
  return Math.round(x).toLocaleString("en-US");
}

function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "style") e.style.cssText = v;
    else if (k === "class") e.className = v;
    else e.setAttribute(k, v);
  }
  for (const c of children) e.append(c instanceof Node ? c : document.createTextNode(String(c)));
  return e;
}

const COLORS = { aws: "--s1", azure: "--s2", aws_glacier_ir: "--s3", azure_cold: "--s4", on_prem: "--s5", satchel_ec2: "--s6" };
const color = (key) => `var(${COLORS[key] || "--muted"})`;

// ---------- inputs <-> request ----------

function startUnit(bytesValue) {
  if (bytesValue >= 1e15) return 1e15;
  if (bytesValue >= 1e12) return 1e12;
  return 1e9;
}

function fill(r) {
  const unit = startUnit(r.workload.start_bytes);
  $("start_unit").value = String(unit);
  $("start").value = +(r.workload.start_bytes / unit).toPrecision(12);
  $("growth").value = Math.round(r.workload.growth * 100);
  $("years").value = r.workload.years;
  $("block").value = +(r.workload.avg_block_bytes / 1e6).toPrecision(6);
  $("egress").value = r.workload.egress_ratio;
  $("cold_read").value = Math.round(r.workload.cold_read_fraction * 100);
  $("scheme").value = r.scheme.preset;
  const custom = r.scheme.preset === "custom" ? r.scheme : { open_replicas: 4, k: 6, m: 3, regions: 2 };
  $("replicas").value = custom.open_replicas;
  $("k").value = custom.k;
  $("m").value = custom.m;
  $("regions").value = custom.regions;
  $("custom").hidden = r.scheme.preset !== "custom";
  $("cloud_regions").value = String(r.cloud_regions);
  $("server").value = r.on_prem.server_usd_per_raw_tb;
  $("life").value = r.on_prem.life_years;
  $("wacc").value = +(r.on_prem.cost_of_capital * 100).toPrecision(6);
  $("engineers").value = r.on_prem.engineers;
  $("technicians").value = r.on_prem.technicians_on_site;
  outputs();
}

function outputs() {
  $("growth_out").value = `${$("growth").value}%`;
  $("cold_read_out").value = `${$("cold_read").value}%`;
}

function num(id) {
  const v = Number($(id).value);
  if (!Number.isFinite(v) || $(id).value === "") throw new Error(`${$(id).closest("label").firstChild.textContent.trim()} needs a number`);
  return v;
}

function read() {
  const preset = $("scheme").value;
  const scheme = preset === "custom"
    ? { preset, open_replicas: num("replicas"), k: num("k"), m: num("m"), regions: num("regions") }
    : { preset };
  return {
    workload: {
      start_bytes: num("start") * Number($("start_unit").value),
      growth: num("growth") / 100,
      years: Math.round(num("years")),
      avg_block_bytes: num("block") * 1e6,
      egress_ratio: num("egress"),
      cold_read_fraction: num("cold_read") / 100,
    },
    scheme,
    on_prem: {
      server_usd_per_raw_tb: num("server"),
      life_years: Math.round(num("life")),
      cost_of_capital: num("wacc") / 100,
      engineers: num("engineers"),
      technicians_on_site: num("technicians"),
    },
    cloud_regions: Number($("cloud_regions").value),
    sensitivity: false,
  };
}

// URL state: only what differs from the base case, as short keys. No
// personal data: these are model inputs only.
const URL_KEYS = {
  start: (r) => r.workload.start_bytes, growth: (r) => r.workload.growth, years: (r) => r.workload.years,
  block: (r) => r.workload.avg_block_bytes, egress: (r) => r.workload.egress_ratio,
  coldread: (r) => r.workload.cold_read_fraction, regions: (r) => r.cloud_regions,
  server: (r) => r.on_prem.server_usd_per_raw_tb, life: (r) => r.on_prem.life_years,
  wacc: (r) => r.on_prem.cost_of_capital, eng: (r) => r.on_prem.engineers, tech: (r) => r.on_prem.technicians_on_site,
};

function toQuery(r) {
  const q = new URLSearchParams();
  for (const [k, get] of Object.entries(URL_KEYS)) if (get(r) !== get(base)) q.set(k, String(get(r)));
  if (r.scheme.preset !== base.scheme.preset) q.set("scheme", r.scheme.preset);
  if (r.scheme.preset === "custom") {
    q.set("rep", r.scheme.open_replicas); q.set("k", r.scheme.k); q.set("m", r.scheme.m); q.set("reg", r.scheme.regions);
  }
  return q.toString();
}

function fromQuery(search) {
  const q = new URLSearchParams(search);
  const r = structuredClone(base);
  const n = (k) => (q.has(k) && Number.isFinite(Number(q.get(k))) ? Number(q.get(k)) : undefined);
  const set = (k, f) => { const v = n(k); if (v !== undefined) f(v); };
  set("start", (v) => (r.workload.start_bytes = v));
  set("growth", (v) => (r.workload.growth = v));
  set("years", (v) => (r.workload.years = v));
  set("block", (v) => (r.workload.avg_block_bytes = v));
  set("egress", (v) => (r.workload.egress_ratio = v));
  set("coldread", (v) => (r.workload.cold_read_fraction = v));
  set("regions", (v) => (r.cloud_regions = v));
  set("server", (v) => (r.on_prem.server_usd_per_raw_tb = v));
  set("life", (v) => (r.on_prem.life_years = v));
  set("wacc", (v) => (r.on_prem.cost_of_capital = v));
  set("eng", (v) => (r.on_prem.engineers = v));
  set("tech", (v) => (r.on_prem.technicians_on_site = v));
  const preset = q.get("scheme");
  if (["magic_pocket", "magic_pocket_cold", "satchel"].includes(preset)) r.scheme = { preset };
  if (preset === "custom") {
    r.scheme = { preset, open_replicas: n("rep") ?? 4, k: n("k") ?? 6, m: n("m") ?? 3, regions: n("reg") ?? 2 };
  }
  return r;
}

// ---------- drawing ----------

function drawTotals(res) {
  const box = $("totals");
  box.replaceChildren();
  const cheapest = res.providers.reduce((a, b) => (b.total_usd < a.total_usd ? b : a));
  for (const p of res.providers) {
    box.append(el("div", { class: `card${p === cheapest ? " cheapest" : ""}`, style: `--c: ${color(p.key)}` },
      el("div", { class: "name" }, p.name),
      el("div", { class: "value" }, usd(p.total_usd)),
      el("div", { class: "name" }, `${usd(p.total_usd / p.yearly_usd.length)} a year on average`)));
  }
  for (const [key, why] of res.skipped) {
    box.append(el("div", { class: "card" }, el("div", { class: "name" }, key), el("div", { class: "name" }, `Not priced: ${why}`)));
  }
  const be = Object.fromEntries(res.break_even.map((b) => [b.cloud, b.start_bytes]));
  const say = (b) => (b == null ? "never, between 1 TB and 10 EB" : `from about ${bytes(b)} of starting data`);
  const start = current.workload.start_bytes;
  const verdict = be.aws != null && start >= be.aws ? "At this size, owning is cheaper than AWS." : "At this size, renting from AWS is cheaper.";
  $("break_even").textContent = `Owning beats AWS ${say(be.aws)}, and Azure ${say(be.azure)}. ${verdict} Cheapest here: ${cheapest.name}.`;
}

function drawChart(res) {
  const svg = $("chart");
  const W = svg.clientWidth || 640;
  const H = 280;
  const pad = { l: 56, r: 12, t: 10, b: 26 };
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  svg.replaceChildren();
  const ns = "http://www.w3.org/2000/svg";
  const mk = (tag, attrs) => { const e = document.createElementNS(ns, tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); return e; };
  const series = res.providers.map((p) => {
    let sum = 0;
    return { key: p.key, name: p.name, points: [0, ...p.yearly_usd.map((y) => (sum += y))] };
  });
  const years = series[0].points.length - 1;
  const shown = series.filter((s) => !hidden.has(s.key));
  const max = Math.max(...(shown.length ? shown : series).flatMap((s) => s.points)) || 1;
  const x = (i) => pad.l + (i / years) * (W - pad.l - pad.r);
  const y = (v) => H - pad.b - (v / max) * (H - pad.t - pad.b);
  for (let t = 0; t <= 4; t += 1) {
    const v = (max * t) / 4;
    svg.append(mk("line", { x1: pad.l, x2: W - pad.r, y1: y(v), y2: y(v), class: "grid" }));
    const label = mk("text", { x: pad.l - 6, y: y(v) + 4, "text-anchor": "end" });
    label.textContent = usd(v);
    svg.append(label);
  }
  for (let i = 0; i <= years; i += 1) {
    const label = mk("text", { x: x(i), y: H - 8, "text-anchor": "middle" });
    label.textContent = i === 0 ? "start" : `year ${i}`;
    svg.append(label);
  }
  const legend = $("legend");
  legend.replaceChildren();
  for (const s of series) {
    const toggle = el("button", { type: "button", class: "series", "aria-pressed": String(!hidden.has(s.key)), title: "Show or hide this line" },
      el("i", { style: `background: ${color(s.key)}` }), s.name);
    toggle.addEventListener("click", () => {
      if (hidden.has(s.key)) hidden.delete(s.key);
      else hidden.add(s.key);
      drawChart(last);
    });
    legend.append(toggle);
    if (hidden.has(s.key)) continue;
    const d = s.points.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
    const path = mk("path", { d, fill: "none", stroke: color(s.key), "stroke-width": s.key === "on_prem" ? 3 : 2 });
    const title = document.createElementNS(ns, "title");
    title.textContent = `${s.name}: ${usd(s.points[years])}`;
    path.append(title);
    svg.append(path);
  }
}

function drawCounts(res) {
  const L = res.layout;
  $("at_year").textContent = `(end of year ${current.workload.years}, ${bytes(L.cloud.hot_bytes + L.cloud.cold_bytes)} stored)`;
  const items = [
    ["Blocks", count(L.logical.blocks)],
    ["Buckets", count(L.logical.buckets)],
    ["Volumes", count(L.logical.volumes)],
    ["Fragments", count(L.logical.fragments)],
    ["Raw on-prem", bytes(L.on_prem.raw_bytes)],
    ["Overhead", `${L.logical.overhead.toFixed(2)}x`],
    ["Drives", count(L.on_prem.drives)],
    ["Servers", count(L.on_prem.servers)],
    ["Racks", count(L.on_prem.racks)],
    ["Cells", count(L.on_prem.cells)],
    ["Power budget", `${count(L.on_prem.power_budget_kw)} kW`],
    ["EC2 nodes", `${count(L.ec2.nodes)} ${L.ec2.instance_type}`],
    ["EBS volumes", `${count(L.ec2.nodes)} x ${count(L.ec2.ebs_gib_per_node)} GiB`],
    ["Cloud put requests", count(L.cloud.put_requests)],
  ];
  $("counts").replaceChildren(...items.map(([k, v]) => el("div", { class: "count" }, el("b", {}, v), el("span", {}, k))));
}

function lastYear(res, key) {
  const p = res.providers.find((x) => x.key === key);
  return p ? p.cost.years[p.cost.years.length - 1].components : {};
}

function drawCloud(res) {
  const L = res.layout.cloud;
  const total = L.hot_bytes + L.cold_bytes || 1;
  const box = $("cloud_layout");
  box.replaceChildren();
  for (let r = 1; r <= L.regions; r += 1) {
    const zones = el("div", { class: "row" }, ...["a", "b", "c"].map((z) => el("div", { class: "box" }, `AZ ${z}`)));
    const tiers = [];
    for (const [key, label] of [["aws", "AWS S3"], ["azure", "Azure Blob"]]) {
      const c = lastYear(res, key);
      tiers.push(el("div", { class: "box" }, el("b", {}, label),
        el("div", { class: "tier" }, el("span", {}, `Hot, ${bytes(L.hot_bytes)}`), el("span", {}, `${usd(c.storage_hot)} / yr`)),
        el("div", { class: "bar", style: `width: ${(100 * L.hot_bytes) / total}%` }),
        el("div", { class: "tier" }, el("span", {}, `Cold, ${bytes(L.cold_bytes)}`), el("span", {}, `${usd(c.storage_cold)} / yr`)),
        el("div", { class: "bar", style: `width: ${(100 * L.cold_bytes) / total}%; opacity: 0.6` }),
        el("div", { class: "tier" }, el("span", {}, `Egress, ${bytes(L.egress_bytes)}`), el("span", {}, `${usd(c.egress)} / yr`))));
    }
    box.append(el("div", { class: "box" }, el("b", {}, `Region ${r}: 3 availability zones, replication managed by the provider`), zones, ...tiers));
  }
  box.append(el("p", { class: "muted" }, "Last year of the horizon. The provider spreads each object over its zones; you pay per GB, request and transfer."));
}

function drawOnPrem(res) {
  const L = res.layout.on_prem;
  const per = (x) => Math.ceil(x / Math.max(L.regions, 1));
  const box = $("onprem_layout");
  box.replaceChildren();
  const regions = Math.min(L.regions, 3);
  for (let r = 1; r <= regions; r += 1) {
    box.append(el("div", { class: "box" }, el("b", {}, `Region ${r}`),
      el("div", { class: "row" },
        el("div", { class: "box" }, el("b", {}, count(per(L.cells))), " cells"),
        el("div", { class: "box" }, el("b", {}, count(per(L.racks))), " racks"),
        el("div", { class: "box" }, el("b", {}, count(per(L.servers))), " servers"),
        el("div", { class: "box" }, el("b", {}, count(per(L.drives))), " drives"))));
  }
  if (L.regions > regions) box.append(el("p", { class: "muted" }, `and ${L.regions - regions} more regions`));
  const repair = L.repair_hours_one_server;
  box.append(el("p", { class: "muted" },
    `${bytes(L.raw_bytes)} raw over ${count(L.servers)} servers. NICs ${count(L.nic_gbps)} Gbps in all.` +
    (repair != null ? ` Rebuilding one server takes about ${repair.toFixed(1)} hours.` : "")));
}

function drawSensitivity(res) {
  const rows = res.sensitivity;
  const pb = (b) => (b == null ? "never" : bytes(b));
  const t = el("table", {},
    el("thead", {}, el("tr", {}, el("th", {}, "Change"), el("th", { class: "num" }, "Break-even vs AWS"), el("th", { class: "num" }, "vs Azure"))),
    el("tbody", {}, ...rows.map((r) => el("tr", {}, el("td", {}, r.what), el("td", { class: "num" }, pb(r.break_even_vs_aws_bytes)), el("td", { class: "num" }, pb(r.break_even_vs_azure_bytes))))));
  $("sensitivity").replaceChildren(el("div", { class: "table-wrap" }, t));
}

function drawPrices() {
  const rows = [];
  const walk = (prefix, v) => {
    if (v && typeof v === "object" && "value" in v && "source" in v) {
      rows.push([prefix, String(v.value), v.source]);
    } else if (Array.isArray(v)) {
      if (v.length && v[0] && "usd" in v[0]) {
        rows.push([prefix, v.map((b) => `${b.usd} up to ${b.up_to_gb ?? "any"} GB`).join("; "), ""]);
      } else if (v.length && v[0] && "input" in v[0]) {
        for (const f of v) rows.push([`${prefix}: ${f.input}`, f.value, f.note]);
      }
    } else if (v && typeof v === "object") {
      for (const [k, w] of Object.entries(v)) walk(prefix ? `${prefix}.${k}` : k, w);
    }
  };
  walk("", prices);
  const t = el("table", {},
    el("thead", {}, el("tr", {}, el("th", {}, "Input"), el("th", {}, "Value"), el("th", {}, "Source"))),
    el("tbody", {}, ...rows.map(([k, v, s]) => el("tr", {}, el("td", {}, k), el("td", {}, v), el("td", {}, s)))));
  $("prices").replaceChildren(el("div", { class: "table-wrap" }, t));
}

// ---------- update loop ----------

function update({ push = true } = {}) {
  outputs();
  $("custom").hidden = $("scheme").value !== "custom";
  try {
    current = read();
    const res = JSON.parse(calculate(JSON.stringify(current)));
    last = res;
    $("error").hidden = true;
    drawTotals(res);
    drawChart(res);
    drawCounts(res);
    drawCloud(res);
    drawOnPrem(res);
    $("sensitivity").replaceChildren();
    if (push) {
      const q = toQuery(current);
      history.replaceState(null, "", q ? `?${q}` : location.pathname);
    }
  } catch (e) {
    $("error").textContent = String(e.message || e);
    $("error").hidden = false;
  }
}

function preset(name) {
  const r = structuredClone(base);
  if (name === "lab") {
    r.workload.start_bytes = 50 * GIB;
    r.scheme = { preset: "satchel" };
  } else if (name !== "dropbox") {
    r.workload.start_bytes = Number(name) * PB;
  }
  fill(r);
  update();
}

async function main() {
  await init();
  const d = JSON.parse(defaults());
  base = d.request;
  prices = d.prices;
  fill(fromQuery(location.search));
  drawPrices();
  update({ push: false });

  let timer;
  $("inputs").addEventListener("input", () => {
    outputs();
    clearTimeout(timer);
    timer = setTimeout(update, 60);
  });
  $("inputs").addEventListener("submit", (e) => e.preventDefault());
  $("reset").addEventListener("click", () => preset("dropbox"));
  for (const b of document.querySelectorAll("[data-preset]")) b.addEventListener("click", () => preset(b.dataset.preset));
  $("share").addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(location.href);
      $("share_status").textContent = "Link copied";
    } catch {
      $("share_status").textContent = location.href;
    }
  });
  $("run_sensitivity").addEventListener("click", () => {
    $("sensitivity").textContent = "Working it out...";
    setTimeout(() => {
      try {
        drawSensitivity(JSON.parse(calculate(JSON.stringify({ ...current, sensitivity: true }))));
      } catch (e) {
        $("sensitivity").textContent = String(e.message || e);
      }
    }, 10);
  });
  window.addEventListener("resize", () => update({ push: false }));
}

main().catch((e) => {
  $("error").textContent = `The calculator failed to load: ${e.message || e}`;
  $("error").hidden = false;
});
