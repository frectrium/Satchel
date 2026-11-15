// Satchel website: store files on S3 (rent) or on a local Satchel (own), and
// see what each does with them and what it costs. Costs come from the same
// Rust engine as the CLI, compiled to WASM (crates/satchel-calc).
const CONFIG = window.SATCHEL_CONFIG;
const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

let wasm = null; // { files_cost, calculate, defaults }
let baseRequest = null;
let session = { user: null };
let localUp = false;
let localTimer = null;

// ---------- small helpers ----------

function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v;
    else if (k === "style") e.style.cssText = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else e.setAttribute(k, v);
  }
  for (const c of children) if (c != null) e.append(c instanceof Node ? c : document.createTextNode(String(c)));
  return e;
}

function bytes(x) {
  if (!Number.isFinite(x)) return "n/a";
  const units = [["EB", 1e18], ["PB", 1e15], ["TB", 1e12], ["GB", 1e9], ["MB", 1e6], ["kB", 1e3]];
  for (const [u, v] of units) if (Math.abs(x) >= v) return `${(x / v).toPrecision(3)} ${u}`;
  return `${Math.round(x)} B`;
}

// Binary units, for limits set in MiB and GiB.
function ibytes(x) {
  if (x >= 1073741824) return `${+(x / 1073741824).toFixed(2)} GiB`;
  if (x >= 1048576) return `${+(x / 1048576).toFixed(2)} MiB`;
  return bytes(x);
}

function usd(x) {
  if (!Number.isFinite(x)) return "n/a";
  const a = Math.abs(x);
  if (a >= 1e9) return `$${(x / 1e9).toFixed(2)}B`;
  if (a >= 1e6) return `$${(x / 1e6).toFixed(2)}M`;
  if (a >= 1e3) return `$${(x / 1e3).toFixed(1)}k`;
  if (a >= 1) return `$${x.toFixed(2)}`;
  if (a >= 0.01) return `$${x.toFixed(3)}`;
  if (a === 0) return "$0";
  return `$${x.toPrecision(2)}`;
}

function when(secs) {
  return new Date(secs * 1000).toLocaleString();
}

function base(target) {
  return target === "aws" ? CONFIG.awsApi : CONFIG.localApi;
}

async function api(target, path, opts = {}) {
  const r = await fetch(base(target) + path, {
    credentials: target === "aws" ? "same-origin" : "omit",
    ...opts,
  });
  const text = await r.text();
  let body = null;
  try { body = text ? JSON.parse(text) : null; } catch { body = { error: text }; }
  if (!r.ok) {
    const err = new Error((body && body.error) || `HTTP ${r.status}`);
    err.status = r.status;
    throw err;
  }
  return body;
}

function installCommand() {
  return `curl -fsSL ${CONFIG.installUrl} | SATCHEL_ALLOWED_ORIGINS=${location.origin} bash`;
}

// ---------- routing ----------

function route() {
  const hash = location.hash.replace(/^#\/?/, "");
  const [pathPart, query = ""] = hash.split("?");
  const [page = "home", sub] = pathPart.split("/");
  const params = new URLSearchParams(query);
  const name = ["home", "store", "files", "docs"].includes(page) ? page : "home";
  for (const p of $$("[data-page]")) p.hidden = p.dataset.page !== name;
  for (const a of $$("[data-route]")) a.classList.toggle("active", a.dataset.route === name);
  clearInterval(localTimer);
  if (name === "store") showStore(params.get("target") || localStorageGet("target") || "local");
  if (name === "files") showFiles(params.get("target") || localStorageGet("target") || "local");
  if (name === "docs") {
    const target = sub && document.getElementById(`doc-${sub}`);
    (target || $("#docs")).scrollIntoView();
  } else {
    window.scrollTo(0, 0);
  }
}

function localStorageGet(k) {
  try { return localStorage.getItem(`satchel.${k}`); } catch { return null; }
}
function localStorageSet(k, v) {
  try { localStorage.setItem(`satchel.${k}`, v); } catch { /* private mode */ }
}

function selectTarget(container, target) {
  for (const b of $$(".segmented button", container)) b.setAttribute("aria-selected", String(b.dataset.target === target));
  localStorageSet("target", target);
}

// ---------- session ----------

async function refreshSession() {
  try {
    const me = await api("aws", "/v1/me");
    session.user = me.user;
  } catch {
    session.user = null;
  }
  const box = $("#session");
  box.replaceChildren();
  if (session.user) {
    box.append(`Signed in as ${session.user}`, el("button", { type: "button", onclick: logout }, "Sign out"));
  }
}

async function logout() {
  try { await api("aws", "/v1/logout", { method: "POST" }); } catch { /* already out */ }
  await refreshSession();
  route();
}

// ---------- store page ----------

function showStore(target) {
  selectTarget($("#store"), target);
  $("#target-aws").hidden = target !== "aws";
  $("#target-local").hidden = target !== "local";
  if (target === "aws") showAws();
  else showLocal();
}

async function showAws() {
  await refreshSession();
  $("#aws-login").hidden = !!session.user;
  $("#aws-ready").hidden = !session.user;
  if (session.user) mountWorkspace($("#aws-ready"), "aws");
}

function showLocal() {
  $("#install-cmd").textContent = installCommand();
  const check = async () => {
    const was = localUp;
    try {
      const ctl = new AbortController();
      const t = setTimeout(() => ctl.abort(), 1500);
      const r = await fetch(`${CONFIG.localApi}/v1/info`, { signal: ctl.signal });
      clearTimeout(t);
      localUp = r.ok;
    } catch {
      localUp = false;
    }
    if (localUp !== was || !$("#local-ready").childElementCount) {
      $("#local-missing").hidden = localUp;
      $("#local-ready").hidden = !localUp;
      if (localUp) mountWorkspace($("#local-ready"), "local");
      else $("#local-ready").replaceChildren();
    }
    $("#local-status").textContent = localUp ? "Found Satchel." : "Looking for Satchel on 127.0.0.1:47800...";
  };
  check();
  localTimer = setInterval(check, 2500);
}

async function mountWorkspace(container, target) {
  container.replaceChildren($("#workspace").content.cloneNode(true));
  const ws = $(".workspace", container);
  const drop = $(".drop", ws);
  const input = $("input[type=file]", ws);
  let info = {};
  try { info = await api(target, "/v1/info"); } catch { /* shown by stats */ }
  $(".limit", ws).textContent = info.max_file_bytes
    ? `Up to ${ibytes(info.max_file_bytes)} per file, ${ibytes(info.max_total_bytes)} in total`
    : "No size limits: it's your own disk";
  const queue = async (files) => {
    for (const f of files) await uploadOne(ws, target, f, info);
  };
  input.addEventListener("change", () => queue([...input.files]).then(() => (input.value = "")));
  drop.addEventListener("dragover", (e) => { e.preventDefault(); drop.classList.add("over"); });
  drop.addEventListener("dragleave", () => drop.classList.remove("over"));
  drop.addEventListener("drop", (e) => { e.preventDefault(); drop.classList.remove("over"); queue([...e.dataTransfer.files]); });
  drop.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") input.click(); });
  await refreshPanels(ws, target);
}

function uploadOne(ws, target, file, info) {
  return new Promise((resolve) => {
    const bar = el("i");
    const status = el("span", { class: "muted" }, "uploading");
    const detail = el("div");
    const row = el("div", { class: "upload" },
      el("div", { class: "row" }, el("span", { class: "name" }, file.name), el("span", {}, bytes(file.size))),
      el("div", { class: "progress" }, bar), status, detail);
    $(".uploads", ws).prepend(row);
    if (info.max_file_bytes && file.size > info.max_file_bytes) {
      status.textContent = `Too big: the limit is ${ibytes(info.max_file_bytes)} per file`;
      status.className = "error";
      return resolve();
    }
    const xhr = new XMLHttpRequest();
    xhr.open("POST", `${base(target)}/v1/files?name=${encodeURIComponent(file.name)}`);
    xhr.withCredentials = target === "aws";
    xhr.upload.onprogress = (e) => { if (e.lengthComputable) bar.style.width = `${(100 * e.loaded) / e.total}%`; };
    xhr.onload = async () => {
      let r = {};
      try { r = JSON.parse(xhr.responseText); } catch { /* not JSON */ }
      if (xhr.status !== 201) {
        status.textContent = r.error || `Failed: HTTP ${xhr.status}`;
        status.className = "error";
        if (xhr.status === 401) showAws();
        return resolve();
      }
      bar.style.width = "100%";
      if (r.backend === "satchel") {
        status.textContent = `${r.blocks} blocks: ${r.new_blocks} new, ${r.dedup_blocks} already stored. ${bytes(r.raw_bytes_written)} written to disk (${r.replicas} replicas). ${r.millis} ms.`;
        const squares = el("div", { class: "blocks", title: "Each square is a 4 MiB block. Outlined ones were already stored." });
        const shown = Math.min(r.blocks, 240);
        for (let i = 0; i < shown; i += 1) squares.append(el("i", { class: i < r.new_blocks ? "" : "dup" }));
        if (r.blocks > shown) squares.append(el("span", { class: "muted small" }, ` +${r.blocks - shown}`));
        detail.append(squares);
      } else {
        status.textContent = `1 object of ${bytes(r.bytes)} in S3. ${r.millis} ms. How many copies AWS keeps, and where, is up to AWS.`;
        detail.append(el("span", { class: "object" }, "1 object"));
      }
      status.className = "ok";
      await refreshPanels(ws, target);
      resolve();
    };
    xhr.onerror = () => {
      status.textContent = target === "local" ? "Couldn't reach your Satchel. Is it still running?" : "Network error";
      status.className = "error";
      resolve();
    };
    xhr.send(file);
  });
}

async function refreshPanels(ws, target) {
  let stats;
  try {
    stats = await api(target, "/v1/stats");
  } catch (e) {
    $(".stats", ws).replaceChildren(el("p", { class: "error" }, e.message));
    return;
  }
  drawStats($(".stats", ws), stats);
  drawCost($(".cost", ws), stats);
}

function drawStats(box, s) {
  const rows = s.backend === "satchel"
    ? [
        ["Files", s.files],
        ["Your data", bytes(s.logical_bytes)],
        ["Distinct blocks", s.unique_blocks],
        ["Stored once (after dedup)", bytes(s.unique_bytes)],
        ["Saved by dedup", bytes(s.dedup_saved_bytes)],
        ["Raw bytes on disk", bytes(s.raw_bytes)],
        ["Copies", `${s.replicas}, one per zone`],
      ]
    : [
        ["Files (objects)", s.files],
        ["Your data", bytes(s.logical_bytes)],
        ["Copies", "managed by AWS"],
        ["Raw bytes on disk", "not disclosed"],
      ];
  const dl = el("dl", { class: "kv" });
  for (const [k, v] of rows) dl.append(el("dt", {}, k), el("dd", {}, String(v)));
  box.replaceChildren(el("h2", {}, s.backend === "satchel" ? "Your Satchel" : "S3 bucket (ap-south-1)"), dl);
  if (s.nodes && s.nodes.length) {
    const max = Math.max(1, ...s.nodes.map((n) => n.bytes));
    box.append(el("div", { class: "nodes" }, ...s.nodes.map((n) =>
      el("div", { class: `nodebox${n.up ? "" : " down"}` },
        el("b", {}, n.zone), " ", n.id, el("div", {}, n.up ? bytes(n.bytes) : "down"),
        el("div", { class: "bar" }, el("i", { style: `width: ${(100 * n.bytes) / max}%` }))))));
  }
}

function drawCost(box, s) {
  if (!wasm) {
    box.replaceChildren(el("p", { class: "muted" }, "Loading the cost engine..."));
    return;
  }
  const c = JSON.parse(wasm.files_cost(JSON.stringify({ logical_bytes: s.logical_bytes, objects: s.files })));
  const sliderOut = el("p", { class: "small" });
  const slider = el("input", { type: "range", min: "0", max: "9", step: "0.1", value: "0", "aria-label": "Scale up this data" });
  const scaled = el("div");
  const update = () => {
    const n = 10 ** Number(slider.value);
    const data = Math.max(s.logical_bytes, 1e6) * n;
    sliderOut.textContent = `${n < 10 ? n.toFixed(1) : Math.round(n).toLocaleString("en-US")} x this data = ${bytes(data)}`;
    try {
      const req = structuredClone(baseRequest);
      req.workload.start_bytes = data;
      const r = JSON.parse(wasm.calculate(JSON.stringify(req)));
      const total = (k) => r.providers.find((p) => p.key === k).total_usd;
      const be = r.break_even.find((b) => b.cloud === "aws").start_bytes;
      const owning = total("on_prem") < total("aws");
      scaled.replaceChildren(
        el("div", { class: "costgrid" },
          el("div", { class: "costbox rent" }, el("div", { class: "muted small" }, `AWS S3, ${r.providers[0].yearly_usd.length} years`), el("div", { class: "big" }, usd(total("aws")))),
          el("div", { class: "costbox own" }, el("div", { class: "muted small" }, "Owning, with staff and network"), el("div", { class: "big" }, usd(total("on_prem"))))),
        el("p", { class: "small" }, owning ? "At this size, owning is cheaper. " : "At this size, renting is cheaper. ",
          be == null ? "" : `With the base-case assumptions, owning beats S3 from about ${bytes(be)} of starting data.`));
    } catch (e) {
      scaled.replaceChildren(el("p", { class: "error" }, String(e.message || e)));
    }
  };
  slider.addEventListener("input", update);
  box.replaceChildren(
    el("h2", {}, "What these files cost"),
    el("div", { class: "costgrid" },
      el("div", { class: "costbox rent" },
        el("div", { class: "muted small" }, "Rent: S3 Standard, per month"),
        el("div", { class: "big" }, usd(c.s3.storage_usd_month)),
        el("div", { class: "muted small" }, `${c.s3.billed_gb.toPrecision(3)} GB at $${c.s3.usd_per_gb_month}/GB [${c.s3.source}], plus ${usd(c.s3.puts_usd)} once for uploads`)),
      el("div", { class: "costbox own" },
        el("div", { class: "muted small" }, "Own: Satchel on-prem, per month"),
        el("div", { class: "big" }, usd(c.satchel.total_usd_month)),
        el("div", { class: "muted small" }, `${bytes(c.satchel.raw_bytes)} raw at ${c.satchel.overhead}x (RS(6,3)): hardware ${usd(c.satchel.hardware_usd_month)}, power ${usd(c.satchel.power_usd_month)}, space ${usd(c.satchel.colocation_usd_month)}`))),
    el("p", { class: "muted small" }, "Own is the marginal cost of the capacity only: hardware paid off over its life, drive power and colocation. Staff and network are fixed costs, so they show up in the scaled view below (our inference). Sources: ",
      c.satchel.sources.map(([k, v]) => `${k} [${v}]`).join(", "), "."),
    el("div", { class: "slider" }, el("h2", {}, "Scale it up"), slider, sliderOut, scaled));
  update();
}

// ---------- files page ----------

async function showFiles(target) {
  selectTarget($("#files"), target);
  const body = $("#files-body");
  body.replaceChildren(el("p", { class: "muted" }, "Loading..."));
  if (target === "aws") {
    await refreshSession();
    if (!session.user) {
      body.replaceChildren(el("p", {}, "Sign in as an admin on the ", el("a", { href: "#/store?target=aws" }, "store page"), " to see the S3 files."));
      return;
    }
  }
  let files;
  try {
    files = await api(target, "/v1/files");
  } catch (e) {
    body.replaceChildren(target === "local"
      ? el("p", {}, "Satchel isn't running on this computer. ", el("a", { href: "#/store?target=local" }, "Install or start it"), ".")
      : el("p", { class: "error" }, e.message));
    return;
  }
  if (!files.length) {
    body.replaceChildren(el("p", { class: "muted" }, "No files yet. ", el("a", { href: `#/store?target=${target}` }, "Store some"), "."));
    return;
  }
  const rows = files.map((f) => el("tr", {},
    el("td", {}, f.name),
    el("td", { class: "num" }, bytes(f.size)),
    el("td", { class: "num" }, target === "local" ? f.blocks : "1 object"),
    el("td", {}, when(f.created_at)),
    el("td", {},
      el("a", { href: `${base(target)}/v1/files/${f.id}/content`, download: f.name }, "Download"), " ",
      el("button", {
        type: "button", class: "danger",
        onclick: async (e) => {
          if (!confirm(`Delete ${f.name}?`)) return;
          e.target.disabled = true;
          try {
            await api(target, `/v1/files/${f.id}`, { method: "DELETE" });
            showFiles(target);
          } catch (err) {
            alert(err.message);
            e.target.disabled = false;
          }
        },
      }, "Delete"))));
  body.replaceChildren(el("div", { class: "table-wrap" }, el("table", {},
    el("thead", {}, el("tr", {}, el("th", {}, "Name"), el("th", { class: "num" }, "Size"), el("th", { class: "num" }, target === "local" ? "Blocks" : "Stored as"), el("th", {}, "Uploaded"), el("th", {}, ""))),
    el("tbody", {}, ...rows))));
}

// ---------- start ----------

async function loadWasm() {
  try {
    const mod = await import("./calculator/pkg/satchel_calc.js");
    await mod.default();
    wasm = mod;
    baseRequest = JSON.parse(mod.defaults()).request;
  } catch (e) {
    console.error("cost engine failed to load", e);
  }
}

function main() {
  $("#install-cmd-docs").textContent = installCommand();
  $("#copy-install").addEventListener("click", async () => {
    try { await navigator.clipboard.writeText(installCommand()); $("#copy-install").textContent = "Copied"; } catch { /* select manually */ }
  });
  for (const b of $$("#store .segmented button")) b.addEventListener("click", () => { location.hash = `#/store?target=${b.dataset.target}`; });
  for (const b of $$("#files .segmented button")) b.addEventListener("click", () => { location.hash = `#/files?target=${b.dataset.target}`; });
  $("#login-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    $("#login-error").textContent = "";
    try {
      await api("aws", "/v1/login", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ user: $("#login-user").value, password: $("#login-password").value }),
      });
      $("#login-password").value = "";
      showAws();
    } catch (err) {
      $("#login-error").textContent = err.status === 401 ? "Wrong user name or password" : err.message;
    }
  });
  window.addEventListener("hashchange", route);
  refreshSession();
  route();
  loadWasm().then(() => { if (!$("#store").hidden) route(); });
}

main();
