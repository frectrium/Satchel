import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  api, ApiError, bytes, filesCost, ibytes, installCommand, localIsUp, scaled, upload, usd,
  type FilesCost, type Info, type ScaleResult, type Stats, type Target, type UploadReport,
} from "../lib/core";
import { Arrow, Card, Counter, Icon, Segmented, fadeUp } from "../components/ui";
import { fromDrop, fromInput, type Picked } from "../lib/picked";

export function LoginCard({ onSignedIn }: { onSignedIn: (user: string) => void }) {
  const [user, setUser] = useState("admin");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <motion.div {...fadeUp}>
      <Card style={{ maxWidth: 440 }}>
        <div className="drop-icon" style={{ marginBottom: 12 }}><Icon name="lock" /></div>
        <h2>Admin sign in</h2>
        <p className="muted small">Uploading to S3 costs real money, so only the team's admin can do it. Everyone can use the calculator and a local Satchel.</p>
        <form className="form" onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true); setError("");
          try {
            const me = await api<{ user: string }>("aws", "/v1/login", {
              method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ user, password }),
            });
            setPassword("");
            onSignedIn(me.user);
          } catch (err) {
            setError(err instanceof ApiError && err.status === 401 ? "Wrong user name or password" : String((err as Error).message));
          } finally { setBusy(false); }
        }}>
          <label className="field">User<input value={user} onChange={(e) => setUser(e.target.value)} autoComplete="username" required /></label>
          <label className="field">Password<input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" required /></label>
          <AnimatePresence>{error && <motion.div className="error small" initial={{ opacity: 0, x: -6 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0 }}>{error}</motion.div>}</AnimatePresence>
          <button className="btn btn-primary" disabled={busy}>{busy ? "Signing in..." : "Sign in"} <Arrow /></button>
        </form>
      </Card>
    </motion.div>
  );
}

export function InstallCard() {
  const [copied, setCopied] = useState(false);
  const cmd = installCommand();
  return (
    <motion.div {...fadeUp}>
      <Card style={{ maxWidth: 760 }}>
        <div className="drop-icon" style={{ marginBottom: 12 }}><Icon name="laptop" /></div>
        <h2>Run Satchel on this computer</h2>
        <p className="muted">One command, macOS or Linux. It installs into <code>~/satchel</code>, needs no admin rights and listens only on 127.0.0.1.</p>
        <div className="terminal"><span className="prompt">$ </span>{cmd}</div>
        <div style={{ display: "flex", gap: 10, alignItems: "center", marginTop: 14, flexWrap: "wrap" }}>
          <button className="btn btn-primary btn-sm" onClick={async () => {
            try { await navigator.clipboard.writeText(cmd); setCopied(true); setTimeout(() => setCopied(false), 1600); } catch { /* select it */ }
          }}>{copied ? <><Icon name="check" size={16} /> Copied</> : <><Icon name="copy" size={16} /> Copy command</>}</button>
          <span className="muted small">
            <motion.span className="pulse" animate={{ opacity: [1, 0.25, 1] }} transition={{ repeat: Infinity, duration: 1.4 }} />
            Waiting for Satchel on 127.0.0.1:47800
          </span>
        </div>
        <p className="muted small" style={{ marginTop: 14 }}>Your browser may ask to let this site reach devices on your local network: that's how the page talks to your own Satchel. Chrome and Firefox work; Safari may block it.</p>
      </Card>
    </motion.div>
  );
}

type Job = { id: number; file: File; path: string; progress: number; report?: UploadReport; error?: string };
type Batch = { label: string; total: number; done: number; failed: number };

function UploadItem({ job, target }: { job: Job; target: Target }) {
  const r = job.report;
  return (
    <motion.div layout initial={{ opacity: 0, y: -10, scale: 0.98 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0 }}
      className={`card upload ${target === "aws" ? "rent" : "own"}`}>
      <div className="upload-top">
        <div className="file-icon" style={{ width: 28, height: 34 }} />
        <div className="upload-name" title={job.path}>{job.path}</div>
        <span className="muted small">{bytes(job.file.size)}</span>
      </div>
      {!r && !job.error && (
        <div className="progress"><motion.div animate={{ width: `${Math.round(job.progress * 100)}%` }} transition={{ ease: "easeOut" }} /></div>
      )}
      {job.error && <div className="upload-result error">{job.error}</div>}
      {r && r.backend === "satchel" && (
        <div className="upload-result">
          <div className="tiles" style={{ marginBottom: 8 }} title="Each square is a 4 MiB block. Outlined squares were already stored.">
            {Array.from({ length: Math.min(r.blocks, 120) }, (_, i) => (
              <motion.span key={i} className={`tile ${i < r.new_blocks ? "" : "dup"}`}
                initial={{ scale: 0, rotate: -20 }} animate={{ scale: 1, rotate: 0 }}
                transition={{ delay: Math.min(i * 0.025, 1.2), type: "spring", stiffness: 520, damping: 20 }} />
            ))}
            {r.blocks > 120 && <span className="muted small">+{r.blocks - 120}</span>}
          </div>
          <span><b>{r.blocks}</b> blocks · <b>{r.new_blocks}</b> new · <b>{r.dedup_blocks}</b> already stored · <b>{bytes(r.raw_bytes_written)}</b> on disk across {r.replicas} zones · {r.millis} ms</span>
        </div>
      )}
      {r && r.backend === "s3" && (
        <div className="flow">
          <motion.span className="object-chip" initial={{ x: -20, opacity: 0 }} animate={{ x: 0, opacity: 1 }}><Icon name="file" size={16} /> 1 object</motion.span>
          <motion.span className="flow-arrow" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.2 }}><Arrow /></motion.span>
          <motion.span className="muted small" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.35 }}>
            {bytes(r.bytes)} in S3 in {r.millis} ms. How many copies, and where, is up to AWS.
          </motion.span>
        </div>
      )}
    </motion.div>
  );
}

function WherePanel({ stats }: { stats: Stats }) {
  if (stats.backend === "s3") {
    return (
      <Card>
        <span className="badge badge-rent"><Icon name="cloud" size={14} /> Rent</span>
        <h2 style={{ marginTop: 10 }}>S3 bucket, ap-south-1</h2>
        <dl className="kv">
          <dt>Files</dt><dd><Counter value={stats.files} format={(n) => String(Math.round(n))} /></dd>
          <dt>Your data</dt><dd><Counter value={stats.logical_bytes} format={bytes} /></dd>
          <dt>Copies kept</dt><dd>up to AWS</dd>
          <dt>Raw bytes on disk</dt><dd>not disclosed</dd>
        </dl>
        <div className="bucket">
          <Icon name="cloud" size={28} />
          <div className="bucket-chips">
            <AnimatePresence>
              {Array.from({ length: Math.min(stats.files, 40) }, (_, i) => (
                <motion.span key={i} initial={{ scale: 0 }} animate={{ scale: 1 }} exit={{ scale: 0 }} transition={{ type: "spring", stiffness: 500, damping: 24 }} />
              ))}
            </AnimatePresence>
          </div>
        </div>
      </Card>
    );
  }
  const max = Math.max(1, ...stats.nodes.map((n) => n.bytes));
  const tiles = Math.min(stats.unique_blocks ?? 0, 36);
  return (
    <Card>
      <span className="badge badge-own"><Icon name="laptop" size={14} /> Own</span>
      <h2 style={{ marginTop: 10 }}>Your Satchel</h2>
      <dl className="kv">
        <dt>Files</dt><dd><Counter value={stats.files} format={(n) => String(Math.round(n))} /></dd>
        <dt>Your data</dt><dd><Counter value={stats.logical_bytes} format={bytes} /></dd>
        <dt>Stored once, after dedup</dt><dd><Counter value={stats.unique_bytes ?? 0} format={bytes} /></dd>
        <dt>Saved by dedup</dt><dd><Counter value={stats.dedup_saved_bytes ?? 0} format={bytes} /></dd>
        <dt>Raw bytes on disk</dt><dd><Counter value={stats.raw_bytes ?? 0} format={bytes} /></dd>
      </dl>
      <div className="zones">
        {stats.nodes.map((n) => (
          <div key={n.id} className={`zone node ${n.up ? "" : "down"}`}>
            <div className="zone-head"><span>Zone {n.zone}</span><span>{n.up ? bytes(n.bytes) : "down"}</span></div>
            <div className="tiles">
              <AnimatePresence>
                {Array.from({ length: n.up ? tiles : 0 }, (_, i) => (
                  <motion.span key={i} className="tile" initial={{ y: -16, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ opacity: 0 }}
                    transition={{ type: "spring", stiffness: 420, damping: 26, delay: (i % 12) * 0.015 }} />
                ))}
              </AnimatePresence>
            </div>
            <div className="bar"><motion.div animate={{ width: `${(100 * n.bytes) / max}%` }} /></div>
          </div>
        ))}
      </div>
      {(stats.raw_bytes ?? 0) > 3 * (stats.unique_bytes ?? 0) && (
        <p className="muted small" style={{ marginTop: 12 }}>Raw bytes are above 3x because deleted files' blocks stay on disk: Satchel drops the index entry and doesn't compact, like Magic Pocket (DDL-013).</p>
      )}
    </Card>
  );
}

function CostPanel({ stats }: { stats: Stats }) {
  const [cost, setCost] = useState<FilesCost | null>(null);
  const [exp, setExp] = useState(0);
  const [scale, setScale] = useState<ScaleResult | null>(null);
  const [failed, setFailed] = useState("");
  const data = Math.max(stats.logical_bytes, 1e6) * 10 ** exp;
  useEffect(() => { filesCost(stats.logical_bytes, stats.files).then(setCost).catch((e) => setFailed(String(e))); }, [stats.logical_bytes, stats.files]);
  useEffect(() => {
    const t = setTimeout(() => { scaled(data).then(setScale).catch((e) => setFailed(String(e))); }, 40);
    return () => clearTimeout(t);
  }, [data]);
  if (failed) return <Card><p className="error">The cost engine failed to load: {failed}</p></Card>;
  if (!cost) return <Card><p className="muted">Loading the cost engine...</p></Card>;
  const top = scale ? Math.max(scale.aws, scale.onPrem) : 1;
  const n = 10 ** exp;
  return (
    <Card>
      <h2>What these files cost</h2>
      <p className="muted small">Live, from the same Rust cost engine as the report, compiled to WebAssembly.</p>
      <div className="compare">
        <div className="price rent">
          <div className="label">Rent: S3 Standard</div>
          <div className="value"><Counter value={cost.s3.storage_usd_month} format={usd} /></div>
          <div className="detail">per month · {cost.s3.billed_gb.toPrecision(3)} GB at ${cost.s3.usd_per_gb_month}/GB <span className="cite">[{cost.s3.source}]</span></div>
        </div>
        <div className="price own">
          <div className="label">Own: Satchel on-prem</div>
          <div className="value"><Counter value={cost.satchel.total_usd_month} format={usd} /></div>
          <div className="detail">per month · {bytes(cost.satchel.raw_bytes)} raw at {cost.satchel.overhead}x (RS(6,3))</div>
        </div>
      </div>
      <p className="muted small" style={{ marginTop: 10 }}>
        "Own" is the marginal cost of the capacity: hardware paid off over its life, drive power and space
        ({cost.satchel.sources.map(([k, v]) => `${k} [${v}]`).join(", ")}). Staff and network are fixed costs, so they appear in the scaled view (our inference).
      </p>

      <div style={{ marginTop: 22 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
          <h2 style={{ margin: 0 }}>Scale it up</h2>
          <span className="muted small">{n < 10 ? n.toFixed(1) : Math.round(n).toLocaleString("en-US")}x · {bytes(data)}</span>
        </div>
        <input className="slider" type="range" min={0} max={9} step={0.05} value={exp} onChange={(e) => setExp(Number(e.target.value))} aria-label="Scale up this data" />
        {scale && (
          <>
            <div className="bars">
              <div className="bar-row"><span>S3</span><div className="bar-track"><motion.div className="bar-fill rent" animate={{ width: `${(100 * scale.aws) / top}%` }} /></div><span className="num"><Counter value={scale.aws} format={usd} /></span></div>
              <div className="bar-row"><span>Own</span><div className="bar-track"><motion.div className="bar-fill own" animate={{ width: `${(100 * scale.onPrem) / top}%` }} /></div><span className="num"><Counter value={scale.onPrem} format={usd} /></span></div>
            </div>
            <div className="muted small" style={{ marginTop: 6 }}>Total over {scale.years} years, owning including staff, power, space and network.</div>
            <AnimatePresence mode="wait">
              <motion.div key={scale.onPrem < scale.aws ? "own" : "rent"} className={`verdict ${scale.onPrem < scale.aws ? "own" : "rent"}`}
                initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }}>
                {scale.onPrem < scale.aws ? "At this size, owning is cheaper." : "At this size, renting is cheaper."}
                {scale.breakEven != null && ` Owning beats S3 from about ${bytes(scale.breakEven)} of starting data.`}
              </motion.div>
            </AnimatePresence>
          </>
        )}
      </div>
    </Card>
  );
}

function Workspace({ target, onUnauthorized }: { target: Target; onUnauthorized: () => void }) {
  const [info, setInfo] = useState<Info | null>(null);
  const [stats, setStats] = useState<Stats | null>(null);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [over, setOver] = useState(false);
  const [batch, setBatch] = useState<Batch | null>(null);
  const [note, setNote] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const folderInput = useRef<HTMLInputElement>(null);
  const next = useRef(0);
  const busy = useRef(false);

  const refresh = useCallback(async () => {
    try { setStats(await api<Stats>(target, "/v1/stats")); }
    catch (e) { if (e instanceof ApiError && e.status === 401) onUnauthorized(); }
  }, [target, onUnauthorized]);

  useEffect(() => { api<Info>(target, "/v1/info").then(setInfo).catch(() => {}); refresh(); }, [target, refresh]);

  const run = async (pending: Promise<Picked[]> | Picked[]) => {
    setNote("");
    let picked: Picked[];
    try { picked = await pending; } catch { setNote("Couldn't read that folder."); return; }
    if (picked.length === 0) { setNote("Nothing to upload: the folder is empty."); return; }
    if (busy.current) { setNote("Wait for the current uploads to finish."); return; }
    busy.current = true;
    const folder = picked.length > 1 && picked[0].path.includes("/") ? picked[0].path.split("/")[0] : null;
    const label = folder ? `${folder}/` : picked.length > 1 ? `${picked.length} files` : picked[0].path;
    let done = 0, failed = 0;
    if (picked.length > 1) setBatch({ label, total: picked.length, done, failed });
    for (const { file, path } of picked) {
      const id = next.current++;
      setJobs((j) => [{ id, file, path, progress: 0 }, ...j].slice(0, 12));
      const patch = (p: Partial<Job>) => setJobs((j) => j.map((x) => (x.id === id ? { ...x, ...p } : x)));
      if (info?.max_file_bytes && file.size > info.max_file_bytes) {
        patch({ error: `Too big: the limit is ${ibytes(info.max_file_bytes)} per file` });
        failed += 1;
      } else {
        try {
          const report = await upload(target, file, (progress) => patch({ progress }), path);
          patch({ report, progress: 1 });
          done += 1;
        } catch (e) {
          const err = e as ApiError;
          patch({ error: err.message });
          failed += 1;
          if (err.status === 401) { onUnauthorized(); break; }
        }
      }
      if (picked.length > 1) setBatch({ label, total: picked.length, done, failed });
      if ((done + failed) % 5 === 0) refresh();
    }
    busy.current = false;
    await refresh();
  };

  return (
    <div className="workspace">
      <div className="stack">
        <motion.label className={`drop ${over ? "over" : ""}`} tabIndex={0}
          whileHover={{ scale: 1.005 }} animate={{ scale: over ? 1.015 : 1 }}
          onDragOver={(e) => { e.preventDefault(); setOver(true); }} onDragLeave={() => setOver(false)}
          onDrop={(e) => { e.preventDefault(); setOver(false); run(fromDrop(e.dataTransfer)); }}
          onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") input.current?.click(); }}>
          <input ref={input} type="file" multiple hidden onChange={(e) => { run(fromInput(e.target.files)); e.target.value = ""; }} />
          <motion.div className="drop-icon" animate={{ y: over ? -6 : 0 }}><Icon name="upload" size={26} /></motion.div>
          <b>Drop files or folders here, or click to choose</b>
          <span className="muted small">
            {info?.max_file_bytes ? `Up to ${ibytes(info.max_file_bytes)} per file, ${ibytes(info.max_total_bytes ?? 0)} in total` : "No limits: it's your own disk"}
          </span>
          <button type="button" className="btn btn-ghost btn-sm" onClick={(e) => { e.preventDefault(); e.stopPropagation(); folderInput.current?.click(); }}>
            Choose a folder instead
          </button>
        </motion.label>
        <input ref={folderInput} type="file" multiple hidden {...({ webkitdirectory: "" } as Record<string, string>)}
          onChange={(e) => { run(fromInput(e.target.files)); e.target.value = ""; }} />
        <AnimatePresence>
          {note && <motion.p className="muted small" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} style={{ margin: 0 }}>{note}</motion.p>}
        </AnimatePresence>
        <AnimatePresence>
          {batch && (
            <motion.div className="card upload" initial={{ opacity: 0, y: -8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
              <div className="upload-top">
                <Icon name="file" size={18} />
                <div className="upload-name">{batch.label}</div>
                <span className="muted small">{batch.done + batch.failed} of {batch.total}{batch.failed ? ` · ${batch.failed} failed` : ""}</span>
              </div>
              <div className="progress"><motion.div animate={{ width: `${(100 * (batch.done + batch.failed)) / batch.total}%` }} /></div>
            </motion.div>
          )}
        </AnimatePresence>
        <AnimatePresence initial={false}>
          {jobs.map((j) => <UploadItem key={j.id} job={j} target={target} />)}
        </AnimatePresence>
      </div>
      <div className="stack">
        {stats ? <><WherePanel stats={stats} /><CostPanel stats={stats} /></> : <Card><p className="muted">Loading...</p></Card>}
      </div>
    </div>
  );
}

export default function Store({ target, setTarget, user, setUser }: {
  target: Target; setTarget: (t: Target) => void; user: string | null; setUser: (u: string | null) => void;
}) {
  const [localUp, setLocalUp] = useState<boolean | null>(null);
  useEffect(() => {
    if (target !== "local") return;
    let live = true;
    const check = async () => { const up = await localIsUp(); if (live) setLocalUp(up); };
    check();
    const t = setInterval(check, 2500);
    return () => { live = false; clearInterval(t); };
  }, [target]);

  return (
    <div className="container">
      <div className="page-head">
        <div>
          <span className="eyebrow">{target === "aws" ? "Rent" : "Own"}</span>
          <h1>Store files</h1>
        </div>
        <Segmented id="store" value={target} onChange={setTarget} options={[
          { value: "aws", label: "AWS S3" },
          { value: "local", label: "My Satchel" },
        ]} />
      </div>
      <AnimatePresence mode="wait">
        <motion.div key={target + String(user) + String(localUp)} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -8 }} transition={{ duration: 0.25 }}>
          {target === "aws" && (user ? <Workspace target="aws" onUnauthorized={() => setUser(null)} /> : <LoginCard onSignedIn={setUser} />)}
          {target === "local" && (localUp === null ? <p className="muted">Looking for Satchel...</p> : localUp ? <Workspace target="local" onUnauthorized={() => {}} /> : <InstallCard />)}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}
