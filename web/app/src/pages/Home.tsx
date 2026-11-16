import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useState } from "react";
import { Arrow, Card, Icon, fadeUp } from "../components/ui";

/** Loops the core idea: a file split into blocks, each copied to 3 zones. */
function HeroMock() {
  const [step, setStep] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setStep((s) => (s + 1) % 7), 900);
    return () => clearInterval(t);
  }, []);
  const blocks = Math.min(step, 5);
  return (
    <motion.div className="mock" {...fadeUp} transition={{ ...fadeUp.transition, delay: 0.15 }}>
      <div className="mock-file">
        <div className="file-icon" />
        <div style={{ flex: 1 }}>
          <div style={{ fontWeight: 700 }}>holiday-video.mp4</div>
          <div className="muted small">20 MiB · split into 4 MiB blocks</div>
        </div>
        <span className="badge badge-own">SHA-256</span>
      </div>
      <div className="tiles" style={{ margin: "14px 4px 0", minHeight: 18 }}>
        <AnimatePresence>
          {Array.from({ length: blocks }, (_, i) => (
            <motion.span key={i} className="tile" initial={{ scale: 0, opacity: 0 }} animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0, opacity: 0 }} transition={{ type: "spring", stiffness: 500, damping: 22 }} />
          ))}
        </AnimatePresence>
      </div>
      <div className="zones">
        {["z1", "z2", "z3"].map((z, zi) => (
          <div className="zone" key={z}>
            <div className="zone-head"><span>Zone {z}</span><span>node</span></div>
            <div className="tiles">
              <AnimatePresence>
                {Array.from({ length: blocks }, (_, i) => (
                  <motion.span key={i} className="tile" initial={{ y: -30, opacity: 0 }} animate={{ y: 0, opacity: 1 }}
                    exit={{ opacity: 0 }} transition={{ delay: 0.12 + zi * 0.08, type: "spring", stiffness: 380, damping: 24 }} />
                ))}
              </AnimatePresence>
            </div>
          </div>
        ))}
      </div>
      <div className="muted small" style={{ marginTop: 12 }}>One copy in each zone. Closed volumes become RS(6,3) fragments: 1.5x instead of 3x.</div>
    </motion.div>
  );
}

export default function Home() {
  return (
    <>
      <section className="container hero">
        <motion.div {...fadeUp}>
          <span className="eyebrow">A pocket-sized Dropbox Magic Pocket</span>
          <h1>Rent your storage, or <em>own</em> it?</h1>
          <p className="lede">
            In 2016 Dropbox moved 500 PB of user files off Amazon S3 onto storage it built and runs itself <span className="cite">[B1]</span>.
            Store a few files on each side and see what happens to them, and what they cost.
          </p>
          <div className="hero-cta">
            <a className="btn btn-primary btn-lg" href="#/store?target=local">Store on your own Satchel <Arrow /></a>
            <a className="btn btn-secondary btn-lg" href="calculator/index.html">Open the calculator</a>
          </div>
        </motion.div>
        <HeroMock />
      </section>

      <section className="container section">
        <div className="stats">
          {[
            ["4 MiB", "blocks, content addressed"],
            ["3 zones", "one replica in each"],
            ["RS(6,3)", "erasure coding once a volume closes"],
            ["1 engine", "for the CLI, this site and the report"],
          ].map(([b, s], i) => (
            <motion.div className="stat" key={b} initial={{ opacity: 0, y: 10 }} whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }} transition={{ delay: i * 0.06 }}>
              <b>{b}</b><span>{s}</span>
            </motion.div>
          ))}
        </div>
      </section>

      <section className="container section">
        <h2 className="title">Two ways to keep a file</h2>
        <p className="muted" style={{ maxWidth: 640 }}>Same file, two very different contracts. One bills you by the byte from the first byte. The other costs you up front, then gets cheaper the more you store.</p>
        <div className="choices">
          <motion.a href="#/store?target=aws" className="card choice rent" whileHover={{ y: -4 }} transition={{ type: "spring", stiffness: 400, damping: 30 }}>
            <span className="badge badge-rent"><Icon name="cloud" size={14} /> Rent</span>
            <h3>AWS S3</h3>
            <p>Each file becomes one object in a bucket in Mumbai. AWS decides how many copies to keep and where. You pay per GB-month and per request.</p>
            <span className="go">Store on S3 (admins) <Arrow /></span>
          </motion.a>
          <motion.a href="#/store?target=local" className="card choice own" whileHover={{ y: -4 }} transition={{ type: "spring", stiffness: 400, damping: 30 }}>
            <span className="badge badge-own"><Icon name="laptop" size={14} /> Own</span>
            <h3>Satchel on your computer</h3>
            <p>Install with one command. Files are split into 4 MiB blocks and each block is stored once by its SHA-256, as in Magic Pocket <span className="cite">[B2]</span>, then copied to three nodes, one per zone.</p>
            <span className="go">Store on your own Satchel <Arrow /></span>
          </motion.a>
        </div>
      </section>

      <section className="container section">
        <h2 className="title">How it works</h2>
        <div className="steps">
          <Card className="step"><h3>Split and hash</h3><p>Files become 4 MiB blocks keyed by SHA-256. A block that's already stored isn't stored again.</p></Card>
          <Card className="step"><h3>Write to three zones</h3><p>Each block is appended to one node in every zone, and only counts once all three have fsynced.</p></Card>
          <Card className="step"><h3>Code it later</h3><p>Full volumes are closed and erasure coded, cutting raw storage from 3x to 1.5x. <a href="#/docs/durability">Read more</a></p></Card>
        </div>
      </section>
    </>
  );
}
