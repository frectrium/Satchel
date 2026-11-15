import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useEffect, useState } from "react";
import { ago, api, ApiError, base, bytes, localIsUp, type FileRecord, type Target } from "../lib/core";
import { Arrow, Card, Icon, Segmented } from "../components/ui";
import { InstallCard, LoginCard } from "./Store";

export default function Files({ target, setTarget, user, setUser }: {
  target: Target; setTarget: (t: Target) => void; user: string | null; setUser: (u: string | null) => void;
}) {
  const [files, setFiles] = useState<FileRecord[] | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "login" | "missing" | "error">("loading");
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState<FileRecord | null>(null);
  const [toast, setToast] = useState("");

  const load = useCallback(async () => {
    setState("loading");
    if (target === "aws" && !user) return setState("login");
    if (target === "local" && !(await localIsUp())) return setState("missing");
    try {
      setFiles(await api<FileRecord[]>(target, "/v1/files"));
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
      setToast(`Deleted ${f.name}`);
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
      {state === "ready" && files && files.length > 0 && (
        <div className="file-list">
          <AnimatePresence initial={false}>
            {files.map((f, i) => (
              <motion.div key={f.id} layout initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0, transition: { delay: Math.min(i * 0.03, 0.3) } }}
                exit={{ opacity: 0, x: 30, transition: { duration: 0.2 } }} className="card file-row">
                <div className="file-icon" style={{ width: 28, height: 34 }} />
                <div style={{ minWidth: 0 }}>
                  <div className="name">{f.name}</div>
                  <div className="meta">{bytes(f.size)} · {ago(f.created_at)}</div>
                </div>
                <span className={`badge ${target === "aws" ? "badge-rent" : "badge-own"} hide-sm`}>
                  {target === "aws" ? "1 object" : `${f.blocks} block${f.blocks === 1 ? "" : "s"}`}
                </span>
                <a className="btn btn-ghost icon-btn" href={`${base(target)}/v1/files/${f.id}/content`} download={f.name} title="Download" aria-label={`Download ${f.name}`}><Icon name="download" size={18} /></a>
                <button className="btn btn-ghost btn-danger icon-btn" title="Delete" aria-label={`Delete ${f.name}`} onClick={() => setConfirm(f)}><Icon name="trash" size={18} /></button>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}

      <AnimatePresence>
        {confirm && (
          <motion.div className="modal-back" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={() => setConfirm(null)}>
            <motion.div className="card modal" initial={{ scale: 0.95, y: 10 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, opacity: 0 }} onClick={(e) => e.stopPropagation()}>
              <h2>Delete {confirm.name}?</h2>
              <p className="muted small">
                {target === "aws" ? "The object is removed from S3." : "Blocks no other file uses are released from Satchel's index. Their bytes stay on disk until compaction, which Satchel doesn't do (DDL-013)."}
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
