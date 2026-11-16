import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useEffect, useState } from "react";
import { ago, api, ApiError, base, bytes, localIsUp, type FileRecord, type Stats, type Target } from "../lib/core";
import { Arrow, Card, Icon, Segmented } from "../components/ui";
import { InstallCard, LoginCard } from "./Store";

export default function Files({ target, setTarget, user, setUser }: {
  target: Target; setTarget: (t: Target) => void; user: string | null; setUser: (u: string | null) => void;
}) {
  const [files, setFiles] = useState<FileRecord[] | null>(null);
  const [stats, setStats] = useState<Stats | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "login" | "missing" | "error">("loading");
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState<FileRecord | null>(null);
  const [toast, setToast] = useState("");

  const load = useCallback(async () => {
    setState("loading");
    if (target === "aws" && !user) return setState("login");
    if (target === "local" && !(await localIsUp())) return setState("missing");
    try {
      const [nextFiles, nextStats] = await Promise.all([
        api<FileRecord[]>(target, "/v1/files"), api<Stats>(target, "/v1/stats"),
      ]);
      setFiles(nextFiles); setStats(nextStats);
      setState("ready");
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) { setUser(null); return setState("login"); }
      setError((e as Error).message);
      setState("error");
    }
  }, [target, user, setUser]);

  useEffect(() => { load(); }, [load]);

  const remove = async (f: FileRecord) => {
    setConfirm(null);
    try {
      await api(target, `/v1/files/${f.id}`, { method: "DELETE" });
      setFiles((list) => (list ?? []).filter((x) => x.id !== f.id));
      setStats(await api<Stats>(target, "/v1/stats"));
      setToast(target === "local" ? `Deleted ${f.name}. Its space returns at the next compaction.` : `Deleted ${f.name}`);
      setTimeout(() => setToast(""), 2200);
    } catch (e) {
      setToast((e as Error).message);
      setTimeout(() => setToast(""), 3000);
    }
  };

  return (
    <div className="container">
      <div className="page-head">
        <div><span className="eyebrow">{target === "aws" ? "Rent" : "Own"}</span><h1>My files</h1></div>
        <Segmented id="files" value={target} onChange={setTarget} options={[
          { value: "aws", label: "AWS S3" },
          { value: "local", label: "My Satchel" },
        ]} />
      </div>

      {state === "loading" && <p className="muted">Loading...</p>}
      {state === "login" && <LoginCard onSignedIn={setUser} />}
      {state === "missing" && <InstallCard />}
      {state === "error" && <p className="error">{error}</p>}
      {state === "ready" && files && files.length === 0 && (
        <Card className="empty">
          <div className="drop-icon"><Icon name="file" /></div>
          <h2>No files yet</h2>
          <p className="muted">Store a few and watch what happens to them.</p>
          <a className="btn btn-primary" href={`#/store?target=${target}`}>Store files <Arrow /></a>
        </Card>
      )}
      {state === "ready" && files && files.length > 0 && <FilesState target={target} files={files} stats={stats} onDelete={setConfirm} />}

      <AnimatePresence>
        {confirm && (
          <motion.div className="modal-back" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={() => setConfirm(null)}>
            <motion.div className="card modal" initial={{ scale: 0.95, y: 10 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, opacity: 0 }} onClick={(e) => e.stopPropagation()}>
              <h2>Delete {confirm.name}?</h2>
              <p className="muted small">
                {target === "aws" ? "The object is removed from S3." : "Blocks no other file uses disappear from My files now. Their disk space is reclaimed the next time you run satchel compact (DDL-013)."}
              </p>
              <div className="modal-actions">
                <button className="btn btn-secondary btn-sm" onClick={() => setConfirm(null)}>Cancel</button>
                <button className="btn btn-primary btn-sm" onClick={() => remove(confirm)}>Delete</button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
      <AnimatePresence>{toast && <motion.div className="toast" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 20 }}>{toast}</motion.div>}</AnimatePresence>
    </div>
  );
}

function FilesState({ target, files, stats, onDelete }: { target: Target; files: FileRecord[]; stats: Stats | null; onDelete: (file: FileRecord) => void }) {
  const groups = target === "local" ? [
    { title: "Compacted", note: "RS(6,3), about 1.5x raw storage", files: files.filter((f) => f.storage?.compacted) },
    { title: "Not yet compacted", note: "3 replicated copies, one in each zone", files: files.filter((f) => !f.storage?.compacted) },
  ] : [{ title: "S3 objects", note: "One object per file", files }];
  return <>
    {target === "local" && stats && <Card style={{ marginBottom: 18 }}>
      <h2>Disk across your zones</h2>
      <div className="zones">{stats.zones.map((zone) => <div key={zone.zone} className="zone node">
        <div className="zone-head"><span>Zone {zone.zone}</span><span>{bytes(zone.raw_bytes)}</span></div>
        <div className="bar"><motion.div animate={{ width: `${zone.raw_bytes ? 100 * zone.coded_bytes / zone.raw_bytes : 0}%` }} /></div>
        <div className="muted small">{bytes(zone.coded_bytes)} coded · {bytes(zone.replicated_bytes)} replicated</div>
      </div>)}</div>
      {(stats.reclaimable_bytes ?? 0) > 0 && <p className="muted small" style={{ marginTop: 12 }}>{bytes(stats.reclaimable_bytes ?? 0)} is waiting for the next compaction after deleted files.</p>}
    </Card>}
    {groups.map((group) => group.files.length > 0 && <section key={group.title} style={{ marginBottom: 24 }}>
      <h2>{group.title}</h2><p className="muted small">{group.note}</p>
      <div className="file-list"><AnimatePresence initial={false}>{group.files.map((f, i) => <FileRow key={f.id} f={f} i={i} target={target} onDelete={onDelete} />)}</AnimatePresence></div>
    </section>)}
  </>;
}

function FileRow({ f, i, target, onDelete }: { f: FileRecord; i: number; target: Target; onDelete: (file: FileRecord) => void }) {
  const coded = f.storage?.coded_blocks ?? 0; const replicated = f.storage?.replicated_blocks ?? 0;
  return <motion.div layout initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0, transition: { delay: Math.min(i * 0.03, 0.3) } }} exit={{ opacity: 0, x: 30 }} className="card file-row">
    <div className="file-icon" style={{ width: 28, height: 34 }} /><div style={{ minWidth: 0, flex: 1 }}><div className="name">{f.name}</div><div className="meta">{bytes(f.size)} · {ago(f.created_at)} · {f.blocks} block{f.blocks === 1 ? "" : "s"}</div>
      {target === "local" && <div className="tiles" title="Replicated blocks have a full copy in every zone. Coded blocks have three fragments per zone.">{Array.from({ length: Math.min(f.blocks, 24) }, (_, block) => <span key={block} className={`tile ${block < coded ? "dup" : ""}`} />)}<span className="muted small"> {coded ? `${coded} coded` : "3 copies per block"}{replicated && coded ? ` · ${replicated} replicated` : ""}</span></div>}</div>
    <span className={`badge ${target === "aws" ? "badge-rent" : "badge-own"} hide-sm`}>{target === "aws" ? "1 object" : f.storage?.compacted ? "RS(6,3)" : "replicated"}</span>
    <a className="btn btn-ghost icon-btn" href={`${base(target)}/v1/files/${f.id}/content`} download={f.name} title="Download" aria-label={`Download ${f.name}`}><Icon name="download" size={18} /></a><button className="btn btn-ghost btn-danger icon-btn" title="Delete" aria-label={`Delete ${f.name}`} onClick={() => onDelete(f)}><Icon name="trash" size={18} /></button>
  </motion.div>;
}
