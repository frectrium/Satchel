// The calculator presents values returned by the Rust/WASM cost engine. It
// owns no pricing logic, which keeps the page, report and CLI aligned.
import init, { calculate, defaults } from "./pkg/satchel_calc.js";

const $ = (id) => document.getElementById(id);
const GIB = 1073741824;
const PB = 1e15;
const COLORS = { aws: "#44546a", azure: "#7c3aed", aws_glacier_ir: "#0f766e", azure_cold: "#c2410c", on_prem: "#e2553a" };

let base;
let prices;
let current;
let last;
let slideIndex = 0;

const CLOUD_AND_OWN_KEYS = new Set(["aws", "azure", "aws_glacier_ir", "azure_cold", "on_prem"]);
const visibleProviders = (response) => response.providers.filter((provider) => CLOUD_AND_OWN_KEYS.has(provider.key));
const providerByKey = (response, key) => visibleProviders(response).find((provider) => provider.key === key);
const technicalTotals = (response) => visibleProviders(response).map(({ key, name, total_usd, yearly_usd }) => ({ key, name, total_usd, yearly_usd }));

function usd(value) {
  if (!Number.isFinite(value)) return "n/a";
  const magnitude = Math.abs(value);
  if (magnitude >= 1e9) return `$${(value / 1e9).toFixed(2)}B`;
  if (magnitude >= 1e6) return `$${(value / 1e6).toFixed(1)}M`;
  if (magnitude >= 1e3) return `$${(value / 1e3).toFixed(1)}k`;
  return `$${value.toFixed(0)}`;
}

function bytes(value) {
  if (!Number.isFinite(value)) return "n/a";
  for (const [unit, size] of [["EB", 1e18], ["PB", 1e15], ["TB", 1e12], ["GB", 1e9], ["MB", 1e6]]) {
    if (Math.abs(value) >= size) return `${(value / size).toPrecision(3)} ${unit}`;
  }
  return `${value.toFixed(0)} B`;
}

function count(value) {
  if (!Number.isFinite(value)) return "n/a";
  if (value >= 1e12) return `${(value / 1e12).toPrecision(3)} trillion`;
  if (value >= 1e9) return `${(value / 1e9).toPrecision(3)} billion`;
  if (value >= 1e6) return `${(value / 1e6).toPrecision(3)} million`;
  return Math.round(value).toLocaleString("en-US");
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (key === "class") node.className = value;
    else if (key === "style") node.style.cssText = value;
    else if (key.startsWith("on")) node.addEventListener(key.slice(2), value);
    else node.setAttribute(key, value);
  }
  for (const child of children.flat()) node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  return node;
}

function startUnit(value) {
  if (value >= 1e15) return 1e15;
  if (value >= 1e12) return 1e12;
  return 1e9;
}

function fill(request) {
  const unit = startUnit(request.workload.start_bytes);
  $("start_unit").value = String(unit);
  $("start").value = +(request.workload.start_bytes / unit).toPrecision(12);
  $("growth").value = Math.round(request.workload.growth * 100);
  $("years").value = request.workload.years;
  $("block").value = +(request.workload.avg_block_bytes / 1e6).toPrecision(6);
  $("egress").value = request.workload.egress_ratio;
  $("cold_read").value = Math.round(request.workload.cold_read_fraction * 100);
  $("scheme").value = request.scheme.preset;
  const custom = request.scheme.preset === "custom" ? request.scheme : { open_replicas: 4, k: 6, m: 3, regions: 2 };
  $("replicas").value = custom.open_replicas;
  $("k").value = custom.k;
  $("m").value = custom.m;
  $("regions").value = custom.regions;
  $("cloud_regions").value = String(request.cloud_regions);
  $("server").value = request.on_prem.server_usd_per_raw_tb;
  $("life").value = request.on_prem.life_years;
  $("wacc").value = +(request.on_prem.cost_of_capital * 100).toPrecision(6);
  $("engineers").value = request.on_prem.engineers;
  $("technicians").value = request.on_prem.technicians_on_site;
  outputs();
}

function outputs() {
  $("growth_out").value = `${$("growth").value}%`;
  $("cold_read_out").value = `${$("cold_read").value}%`;
  $("custom").hidden = $("scheme").value !== "custom";
}

function num(id) {
  const input = $(id);
  const value = Number(input.value);
  if (!Number.isFinite(value) || input.value === "") throw new Error(`${input.closest("label").firstChild.textContent.trim()} needs a number`);
  return value;
}

function read() {
  const preset = $("scheme").value;
  return {
    workload: {
      start_bytes: num("start") * Number($("start_unit").value),
      growth: num("growth") / 100,
      years: Math.round(num("years")),
      avg_block_bytes: num("block") * 1e6,
      egress_ratio: num("egress"),
      cold_read_fraction: num("cold_read") / 100,
    },
    scheme: preset === "custom"
      ? { preset, open_replicas: num("replicas"), k: num("k"), m: num("m"), regions: num("regions") }
      : { preset },
    on_prem: {
      server_usd_per_raw_tb: num("server"), life_years: Math.round(num("life")),
      cost_of_capital: num("wacc") / 100, engineers: num("engineers"), technicians_on_site: num("technicians"),
    },
    cloud_regions: Number($("cloud_regions").value), sensitivity: false,
  };
}

const URL_KEYS = {
  start: (r) => r.workload.start_bytes, growth: (r) => r.workload.growth, years: (r) => r.workload.years,
  block: (r) => r.workload.avg_block_bytes, egress: (r) => r.workload.egress_ratio, coldread: (r) => r.workload.cold_read_fraction,
  regions: (r) => r.cloud_regions, server: (r) => r.on_prem.server_usd_per_raw_tb, life: (r) => r.on_prem.life_years,
  wacc: (r) => r.on_prem.cost_of_capital, eng: (r) => r.on_prem.engineers, tech: (r) => r.on_prem.technicians_on_site,
};

function toQuery(request) {
  const query = new URLSearchParams();
  for (const [key, get] of Object.entries(URL_KEYS)) if (get(request) !== get(base)) query.set(key, String(get(request)));
  if (request.scheme.preset !== base.scheme.preset) query.set("scheme", request.scheme.preset);
  if (request.scheme.preset === "custom") {
    query.set("rep", request.scheme.open_replicas); query.set("k", request.scheme.k); query.set("m", request.scheme.m); query.set("reg", request.scheme.regions);
  }
  return query.toString();
}

function fromQuery(search) {
  const query = new URLSearchParams(search);
  const request = structuredClone(base);
  const number = (key) => (query.has(key) && Number.isFinite(Number(query.get(key))) ? Number(query.get(key)) : undefined);
  const set = (key, put) => { const value = number(key); if (value !== undefined) put(value); };
  set("start", (value) => { request.workload.start_bytes = value; });
  set("growth", (value) => { request.workload.growth = value; });
  set("years", (value) => { request.workload.years = value; });
  set("block", (value) => { request.workload.avg_block_bytes = value; });
  set("egress", (value) => { request.workload.egress_ratio = value; });
  set("coldread", (value) => { request.workload.cold_read_fraction = value; });
  set("regions", (value) => { request.cloud_regions = value; });
  set("server", (value) => { request.on_prem.server_usd_per_raw_tb = value; });
  set("life", (value) => { request.on_prem.life_years = value; });
  set("wacc", (value) => { request.on_prem.cost_of_capital = value; });
  set("eng", (value) => { request.on_prem.engineers = value; });
  set("tech", (value) => { request.on_prem.technicians_on_site = value; });
  const preset = query.get("scheme");
  if (["magic_pocket", "magic_pocket_cold", "satchel"].includes(preset)) request.scheme = { preset };
  if (preset === "custom") request.scheme = {
    preset,
    open_replicas: number("rep") === undefined ? 4 : number("rep"),
    k: number("k") === undefined ? 6 : number("k"),
    m: number("m") === undefined ? 3 : number("m"),
    regions: number("reg") === undefined ? 2 : number("reg"),
  };
  return request;
}

function cards(providers) {
  const cheapest = providers.reduce((first, item) => item.total_usd < first.total_usd ? item : first);
  return el("div", { class: "cards" }, providers.map((provider) => el("div", { class: `card${provider.key === cheapest.key ? " winner" : ""}`, style: `--color:${COLORS[provider.key]}` },
    el("span", {}, provider.name), el("b", {}, usd(provider.total_usd)), el("span", {}, "over the horizon"))));
}

function scaleSlide(response) {
  const data = current.workload.start_bytes;
  const photoCount = data / 4e6;
  const driveCount = data / 1e12;
  const blockCount = data / current.workload.avg_block_bytes;
  const tiles = Array.from({ length: 36 }, () => el("i"));
  return ["1 of 4 · Sense of scale", el("div", {},
    el("h2", {}, `${bytes(data)} is too large to feel. Put it next to familiar things.`),
    el("p", { class: "lede" }, "These illustrations use stated assumptions. They are scale cues, not a capacity plan."),
    el("div", { class: "scale" },
      el("div", { class: "scale-card" }, el("b", {}, count(photoCount)), " 4 MB phone-photo-sized files", el("small", {}, "4 MB per photo is an illustrative assumption. Arithmetic: data ÷ 4 MB (our calc)."), el("div", { class: "blocks" }, tiles)),
      el("div", { class: "scale-card" }, el("b", {}, count(driveCount)), " 1 TB laptop drives", el("small", {}, "1 TB per drive is an illustrative assumption. Arithmetic: data ÷ 1 TB (our calc).")),
      el("div", { class: "scale-card" }, el("b", {}, count(blockCount)), " Satchel blocks at the selected average size", el("small", {}, "Satchel blocks are content-addressed and at most 4 MiB [B2]. Arithmetic: data ÷ average block (our calc)."))
    ))];
}

function totalsSlide(response) {
  const providers = visibleProviders(response);
  const cheapest = providers.reduce((first, item) => item.total_usd < first.total_usd ? item : first);
  return ["2 of 4 · Total cost", el("div", {}, el("h2", {}, `Over ${current.workload.years} years, ${cheapest.name} is the lowest total here.`),
    el("p", { class: "lede" }, "Each cloud row uses its hot and cold access option. The owned row models hardware, facilities and people, not a Satchel deployment on rented virtual machines."), cards(providers))];
}

function yearlySlide(response) {
  const providers = visibleProviders(response);
  const finalYear = (provider) => provider.yearly_usd[provider.yearly_usd.length - 1];
  const max = Math.max(...providers.map(finalYear)) || 1;
  const bars = el("div", { class: "bars" });
  for (const provider of providers) {
    const row = el("div", { class: "bar-row" });
    const track = el("div", { class: "track" });
    const width = 100 * finalYear(provider) / max;
    track.append(el("div", { class: "fill", style: `--color:${COLORS[provider.key]};width:${width}%` }));
    row.append(el("span", {}, provider.name), track, el("span", { class: "number" }, usd(finalYear(provider))));
    bars.append(row);
  }
  return ["3 of 4 · Cost over time", el("div", {},
    el("h2", {}, `What one year costs at the end of year ${current.workload.years}.`),
    el("p", { class: "lede" }, "Cloud bills track stored data. Owned infrastructure mixes variable capacity with fixed operating costs."),
    bars)];
}

function breakEvenSlide(response) {
  const breakEven = Object.fromEntries(response.break_even.map((item) => [item.cloud, item.start_bytes]));
  const say = (value) => value == null ? "No break-even between 1 TB and 10 EB" : `About ${bytes(value)}`;
  const aws = providerByKey(response, "aws");
  const own = providerByKey(response, "on_prem");
  const verdict = aws && own && own.total_usd <= aws.total_usd ? "At this size, owned storage is below AWS in the model." : "At this size, AWS is below owned storage in the model.";
  return ["4 of 4 · Break-even", el("div", {}, el("h2", {}, verdict), el("p", { class: "lede" }, "Break-even is the smallest starting data size where the owned model costs no more over this horizon. Change an advanced assumption to see what moves it."),
    el("div", { class: "break" }, el("div", {}, "Against AWS S3", el("b", {}, say(breakEven.aws))), el("div", {}, "Against Azure Blob", el("b", {}, say(breakEven.azure)))) )];
}

function drawStory(response) {
  const slides = [scaleSlide(response), totalsSlide(response), yearlySlide(response), breakEvenSlide(response)];
  slideIndex = Math.min(slideIndex, slides.length - 1);
  const [label, content] = slides[slideIndex];
  $("slide_label").textContent = label;
  $("slide").replaceChildren(content);
  const dots = $("dots");
  dots.replaceChildren();
  slides.forEach((_, index) => {
    const dot = el("button", { type: "button", class: index === slideIndex ? "on" : "", "aria-label": `Show result ${index + 1}` });
    dot.addEventListener("click", () => { slideIndex = index; drawStory(response); });
    dots.append(dot);
  });
}

function drawTechnical(response) {
  const body = $("technical_totals").querySelector("tbody");
  body.replaceChildren();
  for (const provider of technicalTotals(response)) {
    const row = el("tr");
    row.append(el("td", {}, provider.name));
    row.append(el("td", {}, usd(provider.total_usd)));
    row.append(el("td", {}, provider.yearly_usd.map(usd).join(", ")));
    body.append(row);
  }
  const layout = response.layout;
  const rows = [["Blocks", count(layout.logical.blocks)], ["Volumes", count(layout.logical.volumes)], ["Fragments", count(layout.logical.fragments)], ["Raw on-prem", bytes(layout.on_prem.raw_bytes)], ["Overhead", `${layout.logical.overhead.toFixed(2)}x`], ["Servers", count(layout.on_prem.servers)], ["Drives", count(layout.on_prem.drives)], ["Racks", count(layout.on_prem.racks)], ["Cloud PUT requests", count(layout.cloud.put_requests)]];
  $("counts").replaceChildren();
  for (const [label, value] of rows) $("counts").append(el("div", { class: "count" }, el("b", {}, value), el("span", {}, label)));
}

function drawPrices() {
  const rows = [];
  const walk = (prefix, value) => {
    if (value && typeof value === "object" && "value" in value && "source" in value) rows.push([prefix, String(value.value), value.source]);
    else if (Array.isArray(value) && value.length && value[0] && "usd" in value[0]) rows.push([prefix, value.map((band) => `${band.usd} up to ${band.up_to_gb === null ? "any" : band.up_to_gb} GB`).join("; "), ""]);
    else if (value && typeof value === "object") for (const [key, next] of Object.entries(value)) walk(prefix ? `${prefix}.${key}` : key, next);
  };
  walk("", prices);
  const table = el("table");
  const header = el("tr", {}, el("th", {}, "Input"), el("th", {}, "Value"), el("th", {}, "Source"));
  const body = el("tbody");
  for (const [key, value, source] of rows) body.append(el("tr", {}, el("td", {}, key), el("td", {}, value), el("td", {}, source)));
  table.append(el("thead", {}, header), body);
  $("prices").replaceChildren(el("div", { class: "table-wrap" }, table));
}

function update({ push = true } = {}) {
  outputs();
  try {
    current = read();
    last = JSON.parse(calculate(JSON.stringify(current)));
    $("error").hidden = true;
    drawStory(last);
    drawTechnical(last);
    if (push) {
      const query = toQuery(current);
      history.replaceState(null, "", query ? `?${query}` : location.pathname);
    }
  } catch (error) {
    $("error").textContent = String(error.message || error);
    $("error").hidden = false;
  }
}

async function main() {
  await init();
  const defaultsResponse = JSON.parse(defaults());
  base = defaultsResponse.request;
  prices = defaultsResponse.prices;
  fill(fromQuery(location.search));
  drawPrices();
  update({ push: false });
  let timer;
  $("inputs").addEventListener("input", () => { outputs(); clearTimeout(timer); timer = setTimeout(update, 70); });
  $("inputs").addEventListener("submit", (event) => event.preventDefault());
  $("reset").addEventListener("click", () => { fill(structuredClone(base)); slideIndex = 0; update(); });
  $("previous").addEventListener("click", () => { slideIndex = (slideIndex + 3) % 4; drawStory(last); });
  $("next").addEventListener("click", () => { slideIndex = (slideIndex + 1) % 4; drawStory(last); });
  $("technical_mode").addEventListener("change", (event) => { $("story").hidden = event.target.checked; $("technical").hidden = !event.target.checked; });
  $("share").addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(location.href); $("share_status").textContent = "Link copied"; }
    catch { $("share_status").textContent = location.href; }
  });
}

main().catch((error) => { $("error").textContent = `The calculator failed to load: ${error.message || error}`; $("error").hidden = false; });
