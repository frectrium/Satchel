import { useEffect } from "react";
import { installCommand } from "../lib/core";

const SECTIONS: [string, string][] = [
  ["why", "Why Dropbox left S3"],
  ["architecture", "Architecture"],
  ["blocks", "Blocks and extents"],
  ["write", "Writing a file"],
  ["read", "Reading a file"],
  ["durability", "Replication and coding"],
  ["failure", "When a node fails"],
  ["files", "Files on top of blocks"],
  ["cost", "Rent or own"],
  ["install", "Run it yourself"],
  ["api", "Files API"],
];

const C = ({ id }: { id: string }) => <span className="cite">[{id}]</span>;

function Diagram() {
  return (
    <svg className="diagram" viewBox="0 0 760 330" role="img" aria-label="Satchel components: files service, frontend, Block Index, Master and verifier, and one storage node in each of three zones">
      <defs><marker id="arr" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" className="head" /></marker></defs>
      <rect className="box" x="300" y="10" width="160" height="44" rx="10" /><text x="380" y="37">Files service</text>
      <rect className="box" x="300" y="90" width="160" height="44" rx="10" /><text x="380" y="117">Frontend</text>
      <rect className="box" x="40" y="90" width="190" height="44" rx="10" /><text x="135" y="110">Block Index +</text><text x="135" y="126">Replication Table</text>
      <rect className="box" x="530" y="90" width="190" height="44" rx="10" /><text x="625" y="110">Verifier + Master</text><text className="sub" x="625" y="126">scrub, repair, open/close</text>
      <line className="edge" x1="380" y1="54" x2="380" y2="88" markerEnd="url(#arr)" />
      <line className="edge" x1="300" y1="112" x2="232" y2="112" markerEnd="url(#arr)" />
      <line className="edge" x1="530" y1="112" x2="462" y2="112" strokeDasharray="4 4" />
      {[40, 270, 500].map((x, i) => (
        <g key={x}>
          <rect className="zone-box" x={x} y="180" width="220" height="130" rx="12" />
          <text className="sub" x={x + 110} y="200">Zone z{i + 1}</text>
          <rect className="node-box" x={x + 20} y="215" width="180" height="36" rx="8" />
          <text x={x + 110} y="238">storage node</text>
          <text className="sub" x={x + 110} y="285">extents on disk, fsync</text>
        </g>
      ))}
      <line className="edge" x1="350" y1="134" x2="160" y2="213" markerEnd="url(#arr)" />
      <line className="edge" x1="380" y1="134" x2="380" y2="213" markerEnd="url(#arr)" />
      <line className="edge" x1="410" y1="134" x2="600" y2="213" markerEnd="url(#arr)" />
    </svg>
  );
}

export default function Docs({ sub }: { sub?: string }) {
  useEffect(() => {
    const el = sub && document.getElementById(`doc-${sub}`);
    if (el) el.scrollIntoView({ behavior: "smooth" });
    else window.scrollTo(0, 0);
  }, [sub]);
  return (
    <div className="container docs">
      <nav className="toc" aria-label="On this page">
        {SECTIONS.map(([id, label]) => <a key={id} href={`#/docs/${id}`}>{label}</a>)}
      </nav>
      <article className="doc">
        <span className="eyebrow">How it works</span>
        <h1>How Satchel works</h1>
        <p>A short tour of the case and the system. Every claim carries a Source Log ID, like <C id="B2" />, listed in <code>docs/sources/README.md</code> in the repository.</p>

        <h2 id="doc-why">Why Dropbox left S3</h2>
        <p>Dropbox ran its web and metadata services in its own datacenters but kept user file content in Amazon S3. By 2016 it reported more than 500 million signups and 500 PB of user data, up from about 40 PB in 2012, and said that controlling the storage stack could improve both performance and unit economics <C id="B1" />.</p>
        <p>Magic Pocket became a major project in 2013, dark-launched in August 2014 and served more than 90% of user data by October 2015 <C id="B1" />. A large AWS storage commitment that was due to expire set a hard date <C id="T4" /><C id="T5" />. Dropbox's engineers called cost the leading reason <em>at Dropbox's scale</em>, and said third-party infrastructure stays the right choice where its economics or reach fit a workload <C id="T6" />. Dropbox kept a hybrid setup: AWS kept the remaining user data and served regional needs <C id="F1, p. 120" /><C id="T5" /><C id="T8" />.</p>

        <h2 id="doc-architecture">Architecture</h2>
        <p>Magic Pocket is an immutable block store. Files are split into blocks of up to 4 MB, and a block never changes once written. Blocks are replicated when written and erasure coded later, and each one is stored in more than one zone <C id="B2" /><C id="B5" />. Satchel keeps those ideas at laptop scale: one cell over three zones (DDL-005).</p>
        <Diagram />
        <table>
          <thead><tr><th>Satchel</th><th>Magic Pocket</th><th>Job</th></tr></thead>
          <tbody>
            <tr><td>Frontend</td><td>Frontends</td><td>Put, get and delete blocks by hash, with dedup on put (DDL-002)</td></tr>
            <tr><td>Block Index + Replication Table</td><td>Block Index, Replication Table</td><td>Which volume holds a block, and which nodes hold a volume, in one SQLite store (DDL-003)</td></tr>
            <tr><td>Storage node</td><td>OSD</td><td>Append-only extent files, fsync before acknowledging (DDL-011)</td></tr>
            <tr><td>Master (in the verifier)</td><td>Master</td><td>Opens and closes volumes, places replicas one per zone, starts coding (DDL-004, DDL-009)</td></tr>
            <tr><td>Verifier</td><td>Pocket Watch</td><td>Disk Scrubber and Metadata Scanner, repair after a node stays down (DDL-010, DDL-012)</td></tr>
          </tbody>
        </table>

        <h2 id="doc-blocks">Blocks, volumes and extents</h2>
        <p>A Satchel block is immutable content: the files service cuts a file into pieces no larger than 4 MiB, hashes each piece with SHA-256 and uses that hash as the address. The same bytes therefore have the same address, which is how a second upload can reuse an existing block instead of writing it again (DDL-001, DDL-002) <C id="B2" />.</p>
        <p>The Block Index maps that hash to a volume and byte offset. A volume belongs to one 1 GiB bucket, accepts blocks until 64 MiB or ten minutes, then closes. The Master allocates open volumes and keeps that lifecycle out of the put path (DDL-003, DDL-004, DDL-009). On a node, each replica or coded fragment is one append-only extent file. A record stores the hash and length beside the block bytes, so startup can rebuild the node's in-memory lookup by scanning the extent. The node fsyncs the record before it acknowledges it (DDL-011, DDL-018).</p>

        <h2 id="doc-write">Writing a file</h2>
        <ol>
          <li>The files service splits the file into blocks of up to 4 MiB and hashes each with SHA-256 (DDL-001).</li>
          <li>For each block, the frontend asks the Block Index whether it already has that hash. If so, it isn't stored again: dedup (DDL-002).</li>
          <li>Otherwise the frontend appends it to all three replicas of an open volume, one per zone. Each node fsyncs before replying, and all three must return the same offset (DDL-006, DDL-018).</li>
          <li>Only then is the block recorded in the index. If a replica fails, the frontend tries another open volume, the "write to all, no quorum, retry elsewhere" rule Dropbox describes <C id="B2" />.</li>
          <li>The files service records the file's ordered block list (DDL-020).</li>
        </ol>

        <h2 id="doc-read">Reading a file</h2>
        <ol>
          <li>The files service looks up the manifest, then asks the block API for each content hash in its original order (DDL-020).</li>
          <li>The Block Index returns the volume, offset and healthy extent locations. For a replicated open volume, the frontend reads one replica and tries another zone if that request fails (DDL-003, DDL-006).</li>
          <li>For a coded volume, the RS(6,3) read path takes any six of its nine fragments and reconstructs missing data. A whole fragment or one zone can therefore be unavailable (DDL-007). The local install uses three fragment slots on each of its three zone nodes, so it also survives one node loss.</li>
          <li>The files service joins verified blocks into the response. It does not need to understand volume placement or the on-disk extent format.</li>
        </ol>

        <h2 id="doc-durability">Replication, then erasure coding</h2>
        <p>New data is replicated: three full copies, 3x the bytes. When a volume fills (64 MiB) or has been open 10 minutes, the Master closes it but keeps it replicated. Running <code>satchel compact</code> rewrites only the live blocks, validates Reed-Solomon RS(6,3) fragments, commits the new layout and then deletes the old replicas. RS(6,3) has 6 data and 3 parity fragments, three slots per zone, so it uses about 1.5x the bytes and survives any 3 lost fragments (DDL-004, DDL-007, DDL-008, DDL-013).</p>
        <p className="muted">Deleting a file removes it from My files immediately. Its bytes stay on disk until the next compaction, which is why the page shows space waiting to be reclaimed.</p>

        <h2 id="doc-failure">When a node fails</h2>
        <p>Reads try another replica straight away, or reconstruct from six coded fragments. Writes move to another open volume. If a node stays down longer than the repair threshold (15 minutes in Magic Pocket <C id="B2" />, 30 seconds in Satchel's tests), the Master rebuilds its replicas or fragments on healthy nodes after the verifier reports the condition (DDL-010). The verifier's Disk Scrubber reads data back to catch silent corruption, and its Metadata Scanner checks that the index and extents agree (DDL-012) <C id="B3" />.</p>

        <h2 id="doc-files">Files on top of blocks</h2>
        <p>Magic Pocket stores only blocks; Dropbox keeps file metadata in separate systems such as FileJournal <C id="B2" />. Satchel adds a small SQLite manifest so the demo can store whole files: the original path, size, creation time and ordered block list, plus a reference count per block. Deleting a file hides it immediately and only releases a block that no other manifest still references. That layer is a deliberate deviation from Magic Pocket (DDL-020).</p>

        <h2 id="doc-cost">Rent or own</h2>
        <p>Renting costs per GB-month and per request, from the first byte. Owning costs a lot up front and every month, whatever you store: servers, power, space and the engineers who run it. So the cloud wins small and owning wins big (our inference, which the cost model tests). The <a href="calculator/index.html">calculator</a> runs the same cost engine as the report and finds the break-even size for any inputs.</p>
        <p className="muted">The per-file figure on the store page is hardware paid off over its life, drive power and colocation for the raw bytes only. Staff, switches and transit are fixed costs that show up only at scale, so they're in the scaled view and the calculator (our inference).</p>

        <h2 id="doc-install">Run it yourself</h2>
        <div className="terminal"><span className="prompt">$ </span>{installCommand()}</div>
        <p>That installs the latest release into <code>~/satchel</code> and starts the index, three storage nodes (one per simulated zone), the frontend, the verifier with the Master, and the files API on 127.0.0.1:47800. <code>~/satchel/bin/satchel stop</code>, <code>status</code>, <code>logs</code> and <code>uninstall</code> do what they say.</p>

        <h2 id="doc-api">Files API</h2>
        <table>
          <thead><tr><th>Request</th><th>What it does</th></tr></thead>
          <tbody>
            <tr><td><code>POST /v1/files?name=</code></td><td>Store the body as a file</td></tr>
            <tr><td><code>GET /v1/files</code></td><td>List files, newest first</td></tr>
            <tr><td><code>GET /v1/files/&#123;id&#125;</code></td><td>One file and its block list</td></tr>
            <tr><td><code>GET /v1/files/&#123;id&#125;/content</code></td><td>Download the file</td></tr>
            <tr><td><code>DELETE /v1/files/&#123;id&#125;</code></td><td>Delete it, releasing unshared blocks</td></tr>
            <tr><td><code>GET /v1/stats</code></td><td>Files, bytes, dedup savings and raw bytes per node</td></tr>
            <tr><td><code>GET /v1/info</code></td><td>Version, block size, replicas and nodes</td></tr>
          </tbody>
        </table>
      </article>
    </div>
  );
}
