import { useEffect, useMemo, useState } from "preact/hooks";

type SnapshotMeta = { seq: number };
type NodeRow = { id: string; kind: string; process: string; attrs_json: string };
type EdgeRow = { src_id: string; dst_id: string; kind: string; attrs_json: string };
type GraphResponse = { seq: number; nodes: NodeRow[]; edges: EdgeRow[] };

async function j<T>(url: string): Promise<T> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${r.status} ${r.statusText}`);
  return (await r.json()) as T;
}

export function App() {
  const [snapshots, setSnapshots] = useState<SnapshotMeta[]>([]);
  const [selectedSeq, setSelectedSeq] = useState<number>(0);
  const [graph, setGraph] = useState<GraphResponse | null>(null);
  const [stuck, setStuck] = useState<NodeRow[]>([]);
  const [minSecs, setMinSecs] = useState(5);
  const [kindFilter, setKindFilter] = useState<string>("all");
  const [search, setSearch] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function loadSnapshotsAndLatest(selectLatest = false) {
    const [snaps, latest] = await Promise.all([
      j<SnapshotMeta[]>("/api/snapshots"),
      j<{ seq: number }>("/api/snapshot/latest"),
    ]);
    setSnapshots(snaps);
    if (selectLatest || selectedSeq === 0) {
      setSelectedSeq(latest.seq);
      return latest.seq;
    }
    return selectedSeq;
  }

  async function loadSnapshot(seq: number) {
    if (!seq) return;
    setLoading(true);
    setError(null);
    try {
      const [g, s] = await Promise.all([
        j<GraphResponse>(`/api/snapshot/${seq}/graph`),
        j<NodeRow[]>(`/api/snapshot/${seq}/stuck-requests?min_secs=${minSecs}`),
      ]);
      setGraph(g);
      setStuck(s);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadSnapshotsAndLatest(true).then(loadSnapshot).catch((e) => setError(String(e)));
  }, []);

  const nodeKinds = useMemo(() => {
    const set = new Set((graph?.nodes ?? []).map((n) => n.kind));
    return ["all", ...Array.from(set).sort()];
  }, [graph]);

  const filteredNodes = useMemo(() => {
    const q = search.trim().toLowerCase();
    return (graph?.nodes ?? []).filter((n) => {
      if (kindFilter !== "all" && n.kind !== kindFilter) return false;
      if (!q) return true;
      const hay = `${n.id} ${n.kind} ${n.process} ${n.attrs_json}`.toLowerCase();
      return hay.includes(q);
    });
  }, [graph, kindFilter, search]);

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <div class="row card">
        <strong>peeps-web</strong>
        <span style={{ color: "var(--muted)" }}>
          point-in-time graph explorer (manual refresh)
        </span>
      </div>

      <div class="row card">
        <button
          onClick={() => {
            loadSnapshotsAndLatest(false).catch((e) => setError(String(e)));
          }}
        >
          Refresh list
        </button>
        <button
          onClick={() => {
            loadSnapshotsAndLatest(true).then(loadSnapshot).catch((e) => setError(String(e)));
          }}
        >
          Jump to now
        </button>
        <label>snapshot</label>
        <select
          value={String(selectedSeq)}
          onChange={(e) => setSelectedSeq(Number((e.target as HTMLSelectElement).value))}
        >
          <option value="0">(none)</option>
          {snapshots.map((s) => (
            <option key={s.seq} value={String(s.seq)}>
              {s.seq}
            </option>
          ))}
        </select>
        <button onClick={() => loadSnapshot(selectedSeq)}>Load snapshot</button>

        <label>stuck&gt;=</label>
        <input
          type="number"
          min={1}
          value={minSecs}
          onInput={(e) => setMinSecs(Number((e.target as HTMLInputElement).value || 5))}
          style={{ width: 80 }}
        />
        <span>s</span>

        {loading ? <span>loading...</span> : null}
        {error ? <span style={{ color: "#ff8e8e" }}>{error}</span> : null}
      </div>

      <div class="split">
        <div class="card">
          <h3 style={{ marginTop: 0 }}>Snapshot summary</h3>
          <div>
            seq: <code>{graph?.seq ?? 0}</code>
          </div>
          <div>
            nodes: <code>{graph?.nodes.length ?? 0}</code>
          </div>
          <div>
            edges: <code>{graph?.edges.length ?? 0}</code>
          </div>
          <div>
            stuck requests: <code>{stuck.length}</code>
          </div>
        </div>

        <div class="card">
          <h3 style={{ marginTop: 0 }}>Stuck requests</h3>
          {stuck.length === 0 ? (
            <div style={{ color: "var(--muted)" }}>No stuck requests for this threshold.</div>
          ) : (
            <ul>
              {stuck.slice(0, 25).map((r) => (
                <li key={r.id}>
                  <code>{r.id}</code>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      <div class="card">
        <div class="row" style={{ marginBottom: 8 }}>
          <h3 style={{ margin: 0 }}>Nodes</h3>
          <label>kind</label>
          <select value={kindFilter} onChange={(e) => setKindFilter((e.target as HTMLSelectElement).value)}>
            {nodeKinds.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
          <label>search</label>
          <input
            value={search}
            onInput={(e) => setSearch((e.target as HTMLInputElement).value)}
            placeholder="id/kind/process/attrs"
            style={{ minWidth: 280 }}
          />
          <span style={{ color: "var(--muted)" }}>{filteredNodes.length} shown</span>
        </div>
        <table>
          <thead>
            <tr>
              <th>id</th>
              <th>kind</th>
              <th>process</th>
              <th>attrs</th>
            </tr>
          </thead>
          <tbody>
            {filteredNodes.slice(0, 500).map((n) => (
              <tr key={n.id}>
                <td>
                  <code>{n.id}</code>
                </td>
                <td>{n.kind}</td>
                <td>{n.process}</td>
                <td>
                  <code>{n.attrs_json}</code>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div class="card">
        <h3 style={{ marginTop: 0 }}>Edges</h3>
        <table>
          <thead>
            <tr>
              <th>src</th>
              <th>kind</th>
              <th>dst</th>
              <th>attrs</th>
            </tr>
          </thead>
          <tbody>
            {(graph?.edges ?? []).slice(0, 500).map((e, i) => (
              <tr key={`${e.src_id}:${e.kind}:${e.dst_id}:${i}`}>
                <td>
                  <code>{e.src_id}</code>
                </td>
                <td>{e.kind}</td>
                <td>
                  <code>{e.dst_id}</code>
                </td>
                <td>
                  <code>{e.attrs_json}</code>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
