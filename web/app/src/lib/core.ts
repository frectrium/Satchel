// Backends, formatting and the WASM cost engine, shared by every page.

type Config = { awsApi: string; localApi: string; installUrl: string };
declare global {
  interface Window { SATCHEL_CONFIG?: Partial<Config> }
}

export const CONFIG: Config = {
  awsApi: "/api",
  localApi: "http://127.0.0.1:47800",
  installUrl: "https://raw.githubusercontent.com/frectrium/Satchel/main/scripts/install.sh",
  ...(window.SATCHEL_CONFIG ?? {}),
};

export type Target = "aws" | "local";

export type FileRecord = { id: number; name: string; size: number; sha256: string; created_at: number; blocks: number };
export type NodeUsage = { id: string; zone: string; up: boolean; bytes: number; extents: number };
export type Stats = {
  backend: "satchel" | "s3";
  files: number;
  logical_bytes: number;
  unique_blocks: number | null;
  unique_bytes: number | null;
  dedup_saved_bytes: number | null;
  replicas: number | null;
  raw_bytes: number | null;
  nodes: NodeUsage[];
};
export type Info = { backend: string; login_required: boolean; max_file_bytes: number | null; max_total_bytes: number | null };
export type SatchelReport = {
  backend: "satchel"; file: FileRecord; blocks: number; new_blocks: number; dedup_blocks: number;
  new_bytes: number; replicas: number; raw_bytes_written: number; millis: number;
};
export type S3Report = { backend: "s3"; file: FileRecord; objects: number; bytes: number; millis: number };
export type UploadReport = SatchelReport | S3Report;

export function base(target: Target) {
  return target === "aws" ? CONFIG.awsApi : CONFIG.localApi;
}

export class ApiError extends Error {
  constructor(message: string, public status: number) { super(message); }
}

export async function api<T>(target: Target, path: string, init: RequestInit = {}): Promise<T> {
  const r = await fetch(base(target) + path, { credentials: target === "aws" ? "same-origin" : "omit", ...init });
  const text = await r.text();
  let body: unknown = null;
  try { body = text ? JSON.parse(text) : null; } catch { body = { error: text }; }
  if (!r.ok) throw new ApiError((body as { error?: string })?.error ?? `HTTP ${r.status}`, r.status);
  return body as T;
}

export async function localIsUp(): Promise<boolean> {
  try {
    const ctl = new AbortController();
    const t = setTimeout(() => ctl.abort(), 1500);
    const r = await fetch(`${CONFIG.localApi}/v1/info`, { signal: ctl.signal });
    clearTimeout(t);
    return r.ok;
  } catch {
    return false;
  }
}

/** Uploads with progress events, which fetch() can't report. */
export function upload(target: Target, file: File, onProgress: (f: number) => void): Promise<UploadReport> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", `${base(target)}/v1/files?name=${encodeURIComponent(file.name)}`);
    xhr.withCredentials = target === "aws";
    xhr.upload.onprogress = (e) => { if (e.lengthComputable) onProgress(e.loaded / e.total); };
    xhr.onload = () => {
      let body: { error?: string } & Partial<UploadReport> = {};
      try { body = JSON.parse(xhr.responseText); } catch { /* not JSON */ }
      if (xhr.status === 201) resolve(body as UploadReport);
      else reject(new ApiError(body.error ?? `HTTP ${xhr.status}`, xhr.status));
    };
    xhr.onerror = () => reject(new ApiError(target === "local" ? "Couldn't reach your Satchel. Is it still running?" : "Network error", 0));
    xhr.send(file);
  });
}

export function installCommand() {
  return `curl -fsSL ${CONFIG.installUrl} | SATCHEL_ALLOWED_ORIGINS=${location.origin} bash`;
}

// ---------- formatting ----------

export function bytes(x: number | null | undefined): string {
  if (x == null || !Number.isFinite(x)) return "n/a";
  const units: [string, number][] = [["EB", 1e18], ["PB", 1e15], ["TB", 1e12], ["GB", 1e9], ["MB", 1e6], ["kB", 1e3]];
  for (const [u, v] of units) if (Math.abs(x) >= v) return `${(x / v).toPrecision(3)} ${u}`;
  return `${Math.round(x)} B`;
}

export function ibytes(x: number): string {
  if (x >= 1073741824) return `${+(x / 1073741824).toFixed(2)} GiB`;
  if (x >= 1048576) return `${+(x / 1048576).toFixed(2)} MiB`;
  return bytes(x);
}

export function usd(x: number): string {
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

export function ago(secs: number): string {
  const d = Date.now() / 1000 - secs;
  if (d < 60) return "just now";
  if (d < 3600) return `${Math.floor(d / 60)} min ago`;
  if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
  return new Date(secs * 1000).toLocaleDateString();
}

// ---------- cost engine (Rust, compiled to WASM) ----------

type Engine = {
  calculate(req: string): string;
  files_cost(req: string): string;
  defaults(): string;
};
export type FilesCost = {
  s3: { billed_gb: number; usd_per_gb_month: number; storage_usd_month: number; puts_usd: number; source: string };
  satchel: {
    raw_bytes: number; overhead: number; open_raw_bytes: number; hardware_usd_month: number;
    power_usd_month: number; colocation_usd_month: number; total_usd_month: number; sources: [string, string][];
  };
};
export type ScaleResult = { aws: number; onPrem: number; years: number; breakEven: number | null };

let enginePromise: Promise<{ engine: Engine; baseRequest: Record<string, any> }> | null = null;

export function loadEngine() {
  if (!enginePromise) {
    enginePromise = (async () => {
      const url = new URL("calculator/pkg/satchel_calc.js", document.baseURI).href;
      const mod = await import(/* @vite-ignore */ url);
      await mod.default();
      const engine = mod as unknown as Engine;
      return { engine, baseRequest: JSON.parse(engine.defaults()).request };
    })();
  }
  return enginePromise;
}

export async function filesCost(logical: number, objects: number): Promise<FilesCost> {
  const { engine } = await loadEngine();
  return JSON.parse(engine.files_cost(JSON.stringify({ logical_bytes: logical, objects })));
}

export async function scaled(startBytes: number): Promise<ScaleResult> {
  const { engine, baseRequest } = await loadEngine();
  const req = structuredClone(baseRequest);
  req.workload.start_bytes = startBytes;
  const r = JSON.parse(engine.calculate(JSON.stringify(req)));
  const total = (k: string) => r.providers.find((p: { key: string }) => p.key === k).total_usd as number;
  return {
    aws: total("aws"),
    onPrem: total("on_prem"),
    years: r.providers[0].yearly_usd.length,
    breakEven: r.break_even.find((b: { cloud: string }) => b.cloud === "aws").start_bytes,
  };
}

// ---------- routing ----------

export type Route = { page: string; sub?: string; params: URLSearchParams };

export function parseRoute(): Route {
  const hash = location.hash.replace(/^#\/?/, "");
  const [path, query = ""] = hash.split("?");
  const [page = "home", sub] = path.split("/");
  return { page: page || "home", sub, params: new URLSearchParams(query) };
}

export function remember(key: string, value?: string): string | null {
  try {
    if (value !== undefined) localStorage.setItem(`satchel.${key}`, value);
    return localStorage.getItem(`satchel.${key}`);
  } catch {
    return null;
  }
}
