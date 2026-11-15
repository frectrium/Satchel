import { AnimatePresence, motion } from "framer-motion";
import { StrictMode, useCallback, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, loadEngine, parseRoute, remember, type Route, type Target } from "./lib/core";
import { Logo } from "./components/ui";
import Home from "./pages/Home";
import Store from "./pages/Store";
import Files from "./pages/Files";
import Docs from "./pages/Docs";
import "./styles.css";

const NAV: [string, string, string][] = [
  ["home", "#/", "Home"],
  ["store", "#/store", "Store files"],
  ["files", "#/files", "My files"],
  ["calculator", "calculator/index.html", "Calculator"],
  ["docs", "#/docs", "How it works"],
];

function App() {
  const [route, setRoute] = useState<Route>(parseRoute);
  const [user, setUser] = useState<string | null>(null);

  useEffect(() => {
    const on = () => setRoute(parseRoute());
    window.addEventListener("hashchange", on);
    loadEngine().catch(() => {}); // warm the WASM cost engine
    api<{ user: string | null }>("aws", "/v1/me").then((m) => setUser(m.user)).catch(() => setUser(null));
    return () => window.removeEventListener("hashchange", on);
  }, []);

  useEffect(() => { if (route.page !== "docs") window.scrollTo(0, 0); }, [route.page]);

  const target: Target = (route.params.get("target") as Target) || (remember("target") as Target) || "local";
  const setTarget = useCallback((t: Target) => {
    remember("target", t);
    location.hash = `#/${route.page}?target=${t}`;
  }, [route.page]);
  const signOut = async () => {
    try { await api("aws", "/v1/logout", { method: "POST" }); } catch { /* already out */ }
    setUser(null);
  };

  const page = ["home", "store", "files", "docs"].includes(route.page) ? route.page : "home";
  return (
    <>
      <header className="header">
        <div className="container header-inner">
          <a className="brand" href="#/"><Logo /> satchel</a>
          <nav className="nav" aria-label="Main">
            {NAV.map(([key, href, label]) => (
              <a key={key} href={href} className={key === page ? "active" : ""}>
                {key === page && <motion.span layoutId="nav" className="nav-pill" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
                {label}
              </a>
            ))}
          </nav>
          {user && (
            <div className="session">
              <span className="avatar" aria-hidden="true">{user.slice(0, 1).toUpperCase()}</span>
              <span>{user}</span>
              <button className="btn btn-ghost btn-sm" onClick={signOut}>Sign out</button>
            </div>
          )}
        </div>
      </header>
      <main>
        <AnimatePresence mode="wait">
          <motion.div key={page} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.18 }}>
            {page === "home" && <Home />}
            {page === "store" && <Store target={target} setTarget={setTarget} user={user} setUser={setUser} />}
            {page === "files" && <Files target={target} setTarget={setTarget} user={user} setUser={setUser} />}
            {page === "docs" && <Docs sub={route.sub} />}
          </motion.div>
        </AnimatePresence>
      </main>
      <footer className="footer">
        <div className="container footer-inner">
          <span style={{ display: "flex", alignItems: "center", gap: 8 }}><Logo size={20} /> Satchel, a course project on cloud cost and repatriation, modelled on Dropbox Magic Pocket from public sources.</span>
          <a href="https://github.com/frectrium/Satchel">GitHub</a>
        </div>
      </footer>
    </>
  );
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
