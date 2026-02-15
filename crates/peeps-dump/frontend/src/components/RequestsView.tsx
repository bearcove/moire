import ELK from "elkjs/lib/elk.bundled.js";
import type { ElkEdgeSection, ElkExtendedEdge, ElkNode } from "elkjs/lib/elk-api";
import { useEffect, useMemo, useState } from "preact/hooks";
import type { ProcessDump, RequestSnapshot } from "../types";
import { fmtDuration, classNames } from "../util";
import { Expandable } from "./Expandable";
import { ResourceLink } from "./ResourceLink";
import { isActivePath, navigateTo, resourceHref } from "../routes";

interface Props {
  dumps: ProcessDump[];
  filter: string;
  selectedPath: string;
}

const CHAIN_ID_KEY = "peeps.chain_id";
const SPAN_ID_KEY = "peeps.span_id";
const PARENT_SPAN_ID_KEY = "peeps.parent_span_id";

interface FlatRequest extends RequestSnapshot {
  process: string;
  connection: string;
  peer: string;
  chain_id: string | null;
  span_id: string | null;
  parent_span_id: string | null;
}

interface RequestTaskInteraction {
  key: string;
  href: string;
  label: string;
  kind: "lock" | "mpsc" | "oneshot" | "watch" | "once_cell" | "semaphore" | "roam_channel" | "future_wait";
  ageSecs?: number;
  note?: string;
}

type RequestGraphNodeKind = "request" | "task" | "future" | "resource";

interface RequestGraphNode {
  id: string;
  label: string;
  kind: RequestGraphNodeKind;
  href: string | null;
}

interface RequestGraphEdge {
  id: string;
  source: string;
  target: string;
  label: string;
  severity: "warn" | "danger";
}

interface RequestGraphLayoutNode {
  graph: RequestGraphNode;
  x: number;
  y: number;
  width: number;
  height: number;
}

interface RequestGraphLayoutEdge {
  graph: RequestGraphEdge;
  path: string;
  labelX: number;
  labelY: number;
}

interface RequestGraphLayout {
  width: number;
  height: number;
  nodes: RequestGraphLayoutNode[];
  edges: RequestGraphLayoutEdge[];
}

function truncate(s: string, n: number): string {
  if (s.length <= n) return s;
  return `${s.slice(0, n - 1)}…`;
}

function taskKey(process: string, taskId: number): string {
  return `${process}#${taskId}`;
}

function rowSeverity(r: FlatRequest): string {
  if (r.elapsed_secs > 10) return "severity-danger";
  if (r.elapsed_secs > 2) return "severity-warn";
  return "";
}

function meta(r: RequestSnapshot, key: string): string | null {
  return r.metadata?.[key] ?? null;
}

function requestNodeKey(r: FlatRequest): string {
  return `${r.process}::${r.connection}::${r.request_id}`;
}

function RequestLink({
  r,
  selectedPath,
}: {
  r: FlatRequest;
  selectedPath: string;
}) {
  const href = resourceHref({
    kind: "request",
    process: r.process,
    connection: r.connection,
    requestId: r.request_id,
  });
  return (
    <ResourceLink href={href} active={isActivePath(selectedPath, href)} kind="request">
      {r.method_name ?? `method_${r.method_id}`}
    </ResourceLink>
  );
}

function RequestContextTree({
  r,
  interactionsByTask,
  selectedPath,
}: {
  r: FlatRequest;
  interactionsByTask: Map<string, RequestTaskInteraction[]>;
  selectedPath: string;
}) {
  if (r.task_id == null) return <span class="muted">—</span>;
  const key = taskKey(r.process, r.task_id);
  const interactions = [...(interactionsByTask.get(key) ?? [])].sort(
    (a, b) => (b.ageSecs ?? 0) - (a.ageSecs ?? 0),
  );
  const taskHref = resourceHref({ kind: "task", process: r.process, taskId: r.task_id });

  return (
    <details>
      <summary class="mono" style="cursor: pointer">
        {interactions.length} resource interaction(s)
      </summary>
      <div style="padding-top: 6px">
        <div style="margin-bottom: 6px">
          <ResourceLink href={taskHref} active={isActivePath(selectedPath, taskHref)} kind="task">
            {r.task_name ?? "task"} (#{r.task_id})
          </ResourceLink>
        </div>
        {interactions.length > 0 ? (
          <div class="resource-link-list">
            {interactions.map((i) => (
              <ResourceLink key={i.key} href={i.href} active={isActivePath(selectedPath, i.href)} kind={i.kind}>
                {i.label}
              </ResourceLink>
            ))}
          </div>
        ) : (
          <span class="muted">no tracked locks/channels/futures for this task yet</span>
        )}
      </div>
    </details>
  );
}

function RequestContextMini({
  node,
  interactionsByTask,
  selectedPath,
}: {
  node: FlatRequest;
  interactionsByTask: Map<string, RequestTaskInteraction[]>;
  selectedPath: string;
}) {
  if (node.task_id == null) return null;
  const interactions = interactionsByTask.get(taskKey(node.process, node.task_id)) ?? [];
  if (interactions.length === 0) return null;
  return (
    <div style="margin: 4px 0 0 20px">
      <RequestContextTree r={node} interactionsByTask={interactionsByTask} selectedPath={selectedPath} />
    </div>
  );
}

function RequestTreeNode({
  node,
  childrenByParent,
  interactionsByTask,
  selectedPath,
  depth,
  seen,
}: {
  node: FlatRequest;
  childrenByParent: Map<string, FlatRequest[]>;
  interactionsByTask: Map<string, RequestTaskInteraction[]>;
  selectedPath: string;
  depth: number;
  seen: Set<string>;
}) {
  const key = requestNodeKey(node);
  if (seen.has(key)) return null;
  const nextSeen = new Set(seen);
  nextSeen.add(key);
  const kids = [...(childrenByParent.get(key) ?? [])].sort(
    (a, b) => b.elapsed_secs - a.elapsed_secs,
  );

  return (
    <div style={`margin-left: ${depth * 16}px; margin-top: 6px`}>
      <div class={classNames("tree-item", rowSeverity(node))} style="padding: 6px 8px; border-radius: 6px">
        <span
          class={classNames("dir", node.direction === "Outgoing" ? "dir-out" : "dir-in")}
          style="margin-right: 6px"
        >
          {node.direction === "Outgoing" ? "\u2192" : "\u2190"}
        </span>
        <span class="mono">
          <RequestLink r={node} selectedPath={selectedPath} />
        </span>
        <span class="muted" style="margin-left: 8px">
          {node.process} / {node.connection} / {fmtDuration(node.elapsed_secs)}
        </span>
        {node.task_id != null && (
          <span class="mono" style="margin-left: 8px">
            <ResourceLink
              href={resourceHref({ kind: "task", process: node.process, taskId: node.task_id })}
              active={isActivePath(
                selectedPath,
                resourceHref({ kind: "task", process: node.process, taskId: node.task_id }),
              )}
              kind="task"
            >
              #{node.task_id}
            </ResourceLink>
          </span>
        )}
      </div>
      <RequestContextMini node={node} interactionsByTask={interactionsByTask} selectedPath={selectedPath} />
      {kids.map((child) => (
        <RequestTreeNode
          key={requestNodeKey(child)}
          node={child}
          childrenByParent={childrenByParent}
          interactionsByTask={interactionsByTask}
          selectedPath={selectedPath}
          depth={depth + 1}
          seen={nextSeen}
        />
      ))}
    </div>
  );
}

function sectionPath(section: ElkEdgeSection): string | null {
  if (!section.startPoint || !section.endPoint) return null;
  const points = [section.startPoint, ...(section.bendPoints ?? []), section.endPoint];
  return points.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
}

function edgeLabelPos(section: ElkEdgeSection): { x: number; y: number } {
  if (!section.startPoint || !section.endPoint) return { x: 0, y: 0 };
  const points = [section.startPoint, ...(section.bendPoints ?? []), section.endPoint];
  if (points.length === 1) return { x: points[0].x, y: points[0].y };
  const mid = Math.floor((points.length - 1) / 2);
  const a = points[mid];
  const b = points[Math.min(points.length - 1, mid + 1)];
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 - 5 };
}

function RequestInlineGraph({
  r,
  dump,
  interactionsByTask,
}: {
  r: FlatRequest;
  dump: ProcessDump | undefined;
  interactionsByTask: Map<string, RequestTaskInteraction[]>;
}) {
  const [open, setOpen] = useState(false);
  const graph = useMemo(() => {
    if (!dump || r.task_id == null) return { nodes: [] as RequestGraphNode[], edges: [] as RequestGraphEdge[] };
    const nodes = new Map<string, RequestGraphNode>();
    const edges = new Map<string, RequestGraphEdge>();
    const tasksById = new Map(dump.tasks.map((t) => [t.id, t]));

    const addNode = (node: RequestGraphNode) => {
      if (!nodes.has(node.id)) nodes.set(node.id, node);
    };
    const addEdge = (edge: RequestGraphEdge) => {
      if (!edges.has(edge.id)) edges.set(edge.id, edge);
    };

    const reqHref = resourceHref({
      kind: "request",
      process: r.process,
      connection: r.connection,
      requestId: r.request_id,
    });
    const reqNodeId = `req:${requestNodeKey(r)}`;
    addNode({
      id: reqNodeId,
      label: truncate(`${r.method_name ?? `method_${r.method_id}`} (${fmtDuration(r.elapsed_secs)})`, 44),
      kind: "request",
      href: reqHref,
    });

    const rootTaskId = r.task_id;
    const rootTask = tasksById.get(rootTaskId);
    const rootTaskNodeId = `task:${rootTaskId}`;
    addNode({
      id: rootTaskNodeId,
      label: truncate(`${rootTask?.name ?? r.task_name ?? "task"} (#${rootTaskId})`, 44),
      kind: "task",
      href: resourceHref({ kind: "task", process: r.process, taskId: rootTaskId }),
    });
    addEdge({
      id: `edge:req-task:${requestNodeKey(r)}:${rootTaskId}`,
      source: reqNodeId,
      target: rootTaskNodeId,
      label: "handled by",
      severity: "warn",
    });

    // Parent task lineage (limited depth to keep card graph readable).
    let lineageCursor = rootTask;
    let lineageDepth = 0;
    while (lineageCursor?.parent_task_id != null && lineageDepth < 4) {
      const parentId = lineageCursor.parent_task_id;
      const parent = tasksById.get(parentId);
      const parentNodeId = `task:${parentId}`;
      addNode({
        id: parentNodeId,
        label: truncate(`${parent?.name ?? lineageCursor.parent_task_name ?? "task"} (#${parentId})`, 44),
        kind: "task",
        href: resourceHref({ kind: "task", process: r.process, taskId: parentId }),
      });
      addEdge({
        id: `edge:spawn:${parentId}:${lineageCursor.id}`,
        source: parentNodeId,
        target: `task:${lineageCursor.id}`,
        label: "spawns",
        severity: "warn",
      });
      lineageCursor = parent;
      lineageDepth += 1;
    }

    const waits = dump.future_waits
      .filter((w) => w.task_id === rootTaskId)
      .sort((a, b) => b.total_pending_secs - a.total_pending_secs)
      .slice(0, 10);
    const waitFutureIds = new Set<number>();

    for (const w of waits) {
      const futureNodeId = `future:${w.future_id}`;
      waitFutureIds.add(w.future_id);
      addNode({
        id: futureNodeId,
        label: truncate(`future ${w.resource} [#${w.future_id}]`, 56),
        kind: "future",
        href: resourceHref({ kind: "future_wait", process: r.process, taskId: w.task_id, resource: w.resource }),
      });
      addEdge({
        id: `edge:awaits:${rootTaskId}:${w.future_id}`,
        source: rootTaskNodeId,
        target: futureNodeId,
        label: `awaits ${fmtDuration(w.total_pending_secs)}`,
        severity: w.total_pending_secs > 10 ? "danger" : "warn",
      });

      if (w.created_by_task_id != null) {
        const creatorNodeId = `task:${w.created_by_task_id}`;
        addNode({
          id: creatorNodeId,
          label: truncate(`${w.created_by_task_name ?? "task"} (#${w.created_by_task_id})`, 44),
          kind: "task",
          href: resourceHref({ kind: "task", process: r.process, taskId: w.created_by_task_id }),
        });
        addEdge({
          id: `edge:creates:${w.created_by_task_id}:${w.future_id}`,
          source: creatorNodeId,
          target: futureNodeId,
          label: "creates",
          severity: "warn",
        });
      }
    }

    for (const e of dump.future_wake_edges) {
      if (!waitFutureIds.has(e.future_id)) continue;
      const futureNodeId = `future:${e.future_id}`;
      if (e.source_task_id != null) {
        const sourceNodeId = `task:${e.source_task_id}`;
        addNode({
          id: sourceNodeId,
          label: truncate(`${e.source_task_name ?? "task"} (#${e.source_task_id})`, 44),
          kind: "task",
          href: resourceHref({ kind: "task", process: r.process, taskId: e.source_task_id }),
        });
        addEdge({
          id: `edge:wakes:${e.source_task_id}:${e.future_id}`,
          source: sourceNodeId,
          target: futureNodeId,
          label: `wakes x${e.wake_count}`,
          severity: "warn",
        });
      }
      if (e.target_task_id != null) {
        const targetNodeId = `task:${e.target_task_id}`;
        addNode({
          id: targetNodeId,
          label: truncate(`${e.target_task_name ?? "task"} (#${e.target_task_id})`, 44),
          kind: "task",
          href: resourceHref({ kind: "task", process: r.process, taskId: e.target_task_id }),
        });
        addEdge({
          id: `edge:resumes:${e.future_id}:${e.target_task_id}`,
          source: futureNodeId,
          target: targetNodeId,
          label: `resumes x${e.wake_count}`,
          severity: "warn",
        });
      }
    }

    const interactions = interactionsByTask.get(taskKey(r.process, rootTaskId)) ?? [];
    for (const i of interactions.slice(0, 8)) {
      const resNodeId = `resource:${i.key}`;
      addNode({
        id: resNodeId,
        label: truncate(i.label, 56),
        kind: "resource",
        href: i.href,
      });
      addEdge({
        id: `edge:touches:${rootTaskId}:${i.key}`,
        source: rootTaskNodeId,
        target: resNodeId,
        label: "touches",
        severity: "warn",
      });
    }

    return {
      nodes: [...nodes.values()],
      edges: [...edges.values()],
    };
  }, [dump, interactionsByTask, r]);

  const [layout, setLayout] = useState<RequestGraphLayout | null>(null);
  useEffect(() => {
    let cancelled = false;
    async function run() {
      if (!open) return;
      if (graph.nodes.length === 0) {
        setLayout(null);
        return;
      }
      const elk = new ELK();
      const elkNodes: ElkNode[] = graph.nodes.map((n) => ({
        id: n.id,
        width: Math.max(180, Math.min(340, 130 + n.label.length * 4)),
        height: 34,
      }));
      const elkEdges: ElkExtendedEdge[] = graph.edges.map((e) => ({
        id: e.id,
        sources: [e.source],
        targets: [e.target],
      }));
      const graphDef: ElkNode = {
        id: "request-graph",
        layoutOptions: {
          "elk.algorithm": "layered",
          "elk.direction": "RIGHT",
          "elk.edgeRouting": "SPLINES",
          "elk.padding": "[top=16,left=20,bottom=16,right=20]",
          "elk.spacing.nodeNode": "32",
          "elk.layered.spacing.nodeNodeBetweenLayers": "80",
        },
        children: elkNodes,
        edges: elkEdges,
      };
      const out = await elk.layout(graphDef);
      if (cancelled) return;

      const nodeById = new Map(graph.nodes.map((n) => [n.id, n]));
      const edgeById = new Map(graph.edges.map((e) => [e.id, e]));
      const laidNodes: RequestGraphLayoutNode[] = (out.children ?? []).map((n) => ({
        graph: nodeById.get(n.id!)!,
        x: n.x ?? 0,
        y: n.y ?? 0,
        width: n.width ?? 180,
        height: n.height ?? 34,
      }));
      const laidEdges: RequestGraphLayoutEdge[] = [];
      for (const e of out.edges ?? []) {
        const sec = (e.sections ?? [])[0];
        const graphEdge = edgeById.get(e.id!);
        if (!sec || !graphEdge) continue;
        const path = sectionPath(sec);
        if (!path) continue;
        const labelPos = edgeLabelPos(sec);
        laidEdges.push({
          graph: graphEdge,
          path,
          labelX: labelPos.x,
          labelY: labelPos.y,
        });
      }
      setLayout({
        width: out.width ?? 860,
        height: out.height ?? 200,
        nodes: laidNodes,
        edges: laidEdges,
      });
    }
    run().catch((e) => {
      if (!cancelled) {
        console.error("request ELK layout failed", e);
        setLayout(null);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [graph, open]);

  if (graph.nodes.length === 0) {
    return <span class="muted">no graphable causality yet (missing task/future edges)</span>;
  }

  return (
    <details class="sync-details" onToggle={(e) => setOpen((e.currentTarget as HTMLDetailsElement).open)}>
      <summary>Graph (ELK)</summary>
      {open && (
        <div class="causal-graph-wrap" style="padding: 8px 0 4px">
          {!layout && <div class="muted">Computing ELK layout…</div>}
          {layout && (
            <svg
              class="causal-graph"
              viewBox={`0 0 ${Math.max(900, layout.width)} ${Math.max(220, layout.height)}`}
              role="img"
            >
              <defs>
                <marker id="req-elk-arrow-danger" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto">
                  <path d="M0,0 L10,3 L0,6 z" fill="var(--red)" />
                </marker>
                <marker id="req-elk-arrow-warn" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto">
                  <path d="M0,0 L10,3 L0,6 z" fill="var(--amber)" />
                </marker>
              </defs>
              {layout.edges.map((e) => (
                <g key={e.graph.id}>
                  <path
                    d={e.path}
                    class={classNames(
                      "causal-edge",
                      e.graph.severity === "danger" ? "causal-edge-danger" : "causal-edge-warn",
                    )}
                    marker-end={e.graph.severity === "danger" ? "url(#req-elk-arrow-danger)" : "url(#req-elk-arrow-warn)"}
                  />
                  <text class="causal-edge-label" x={e.labelX} y={e.labelY} text-anchor="middle">
                    {e.graph.label}
                  </text>
                </g>
              ))}
              {layout.nodes.map((n) => (
                <g
                  key={n.graph.id}
                  onClick={() => {
                    if (n.graph.href) navigateTo(n.graph.href);
                  }}
                >
                  <rect
                    class={classNames("causal-node", n.graph.kind === "future" || n.graph.kind === "resource" ? "owner" : "blocked")}
                    x={n.x}
                    y={n.y}
                    width={n.width}
                    height={n.height}
                    rx={8}
                    ry={8}
                  />
                  <text class="causal-node-text" x={n.x + n.width / 2} y={n.y + 21} text-anchor="middle">
                    {n.graph.label}
                  </text>
                </g>
              ))}
            </svg>
          )}
        </div>
      )}
    </details>
  );
}

export function RequestsView({ dumps, filter, selectedPath }: Props) {
  const [view, setView] = useState<"table" | "tree">("table");
  const dumpByProcess = new Map(dumps.map((d) => [d.process_name, d]));
  const interactionsByTask = new Map<string, RequestTaskInteraction[]>();
  const addInteraction = (process: string, taskId: number | null, interaction: RequestTaskInteraction) => {
    if (taskId == null) return;
    const key = taskKey(process, taskId);
    const list = interactionsByTask.get(key) ?? [];
    if (!list.some((i) => i.key === interaction.key)) {
      list.push(interaction);
      interactionsByTask.set(key, list);
    }
  };

  for (const d of dumps) {
    for (const w of d.future_waits) {
      addInteraction(d.process_name, w.task_id, {
        key: `future:${w.future_id}:${w.task_id}`,
        href: resourceHref({
          kind: "future_wait",
          process: d.process_name,
          taskId: w.task_id,
          resource: w.resource,
        }),
        label: `future ${w.resource}`,
        kind: "future_wait",
        ageSecs: w.total_pending_secs,
        note: `pending ${fmtDuration(w.total_pending_secs)}`,
      });
    }

    if (d.locks) {
      for (const l of d.locks.locks) {
        const lockHref = resourceHref({ kind: "lock", process: d.process_name, lock: l.name });
        for (const h of l.holders) {
          addInteraction(d.process_name, h.task_id, {
            key: `lock:${l.name}:holder:${h.task_id ?? "?"}`,
            href: lockHref,
            label: `lock ${l.name} (holder)`,
            kind: "lock",
            ageSecs: h.held_secs,
          });
        }
        for (const w of l.waiters) {
          addInteraction(d.process_name, w.task_id, {
            key: `lock:${l.name}:waiter:${w.task_id ?? "?"}`,
            href: lockHref,
            label: `lock ${l.name} (waiter)`,
            kind: "lock",
            ageSecs: w.waiting_secs,
          });
        }
      }
    }

    if (d.sync) {
      for (const ch of d.sync.mpsc_channels) {
        addInteraction(d.process_name, ch.creator_task_id, {
          key: `mpsc:${ch.name}`,
          href: resourceHref({ kind: "mpsc", process: d.process_name, name: ch.name }),
          label: `mpsc ${ch.name}`,
          kind: "mpsc",
          ageSecs: ch.age_secs,
        });
      }
      for (const ch of d.sync.oneshot_channels) {
        addInteraction(d.process_name, ch.creator_task_id, {
          key: `oneshot:${ch.name}`,
          href: resourceHref({ kind: "oneshot", process: d.process_name, name: ch.name }),
          label: `oneshot ${ch.name}`,
          kind: "oneshot",
          ageSecs: ch.age_secs,
        });
      }
      for (const ch of d.sync.watch_channels) {
        addInteraction(d.process_name, ch.creator_task_id, {
          key: `watch:${ch.name}`,
          href: resourceHref({ kind: "watch", process: d.process_name, name: ch.name }),
          label: `watch ${ch.name}`,
          kind: "watch",
          ageSecs: ch.age_secs,
        });
      }
      for (const sem of d.sync.semaphores) {
        addInteraction(d.process_name, sem.creator_task_id, {
          key: `sem:${sem.name}`,
          href: resourceHref({ kind: "semaphore", process: d.process_name, name: sem.name }),
          label: `semaphore ${sem.name}`,
          kind: "semaphore",
          ageSecs: sem.oldest_wait_secs,
        });
      }
    }

    if (d.roam) {
      for (const ch of d.roam.channel_details ?? []) {
        addInteraction(d.process_name, ch.task_id, {
          key: `roam:${ch.channel_id}`,
          href: resourceHref({ kind: "roam_channel", process: d.process_name, channelId: ch.channel_id }),
          label: `roam ${ch.name}`,
          kind: "roam_channel",
          ageSecs: ch.age_secs,
        });
      }
    }
  }

  const requests: FlatRequest[] = [];
  for (const d of dumps) {
    if (!d.roam) continue;
    for (const c of d.roam.connections) {
      for (const r of c.in_flight) {
        requests.push({
          ...r,
          process: d.process_name,
          connection: c.name,
          peer: c.peer_name ?? "?",
          chain_id: meta(r, CHAIN_ID_KEY),
          span_id: meta(r, SPAN_ID_KEY),
          parent_span_id: meta(r, PARENT_SPAN_ID_KEY),
        });
      }
    }
  }
  requests.sort((a, b) => b.elapsed_secs - a.elapsed_secs);

  const q = filter.toLowerCase();
  const filtered = requests.filter(
    (r) =>
      !q ||
      r.process.toLowerCase().includes(q) ||
      (r.method_name?.toLowerCase().includes(q) ?? false) ||
      r.connection.toLowerCase().includes(q) ||
      r.peer.toLowerCase().includes(q) ||
      (r.chain_id?.toLowerCase().includes(q) ?? false) ||
      (r.span_id?.toLowerCase().includes(q) ?? false),
  );

  if (requests.length === 0) {
    return (
      <div class="empty-state fade-in">
        <div class="icon">R</div>
        <p>No in-flight requests</p>
      </div>
    );
  }

  const byChain = new Map<string, FlatRequest[]>();
  for (const r of filtered) {
    const key = r.chain_id ?? `unscoped:${r.process}`;
    const list = byChain.get(key) ?? [];
    list.push(r);
    byChain.set(key, list);
  }
  const chains = [...byChain.entries()].sort((a, b) => {
    const aWorst = Math.max(...a[1].map((r) => r.elapsed_secs), 0);
    const bWorst = Math.max(...b[1].map((r) => r.elapsed_secs), 0);
    return bWorst - aWorst;
  });

  return (
    <div class="fade-in">
      <div style="margin-bottom: 12px; display: flex; gap: 8px">
        <button
          class={classNames("expand-trigger", view === "table" && "active")}
          style="padding: 4px 10px"
          onClick={() => setView("table")}
        >
          Table
        </button>
        <button
          class={classNames("expand-trigger", view === "tree" && "active")}
          style="padding: 4px 10px"
          onClick={() => setView("tree")}
        >
          Tree
        </button>
      </div>

      {view === "table" ? (
        <div style="display: flex; flex-direction: column; gap: 10px">
          {filtered.map((r) => (
            <div
              key={requestNodeKey(r)}
              class="sync-instance-card"
              style={
                rowSeverity(r) === "severity-danger"
                  ? "border-color: var(--red)"
                  : rowSeverity(r) === "severity-warn"
                    ? "border-color: var(--amber)"
                    : undefined
              }
            >
              <div class="sync-instance-head">
                <div class="sync-instance-title mono">
                  <RequestLink r={r} selectedPath={selectedPath} />
                  <span class="muted">{"\u2022"}</span>
                  <ResourceLink
                    href={resourceHref({ kind: "process", process: r.process })}
                    active={isActivePath(selectedPath, resourceHref({ kind: "process", process: r.process }))}
                    kind="process"
                  >
                    {r.process}
                  </ResourceLink>
                  <span class="muted">{"\u2192"}</span>
                  <span>{r.peer}</span>
                </div>
                <div class="mono">{fmtDuration(r.elapsed_secs)}</div>
              </div>

              <div class="sync-instance-body" style="margin-top: 10px">
                <div class="sync-kv">
                  <span class="k">Direction</span>
                  <span class="v">{r.direction}</span>
                </div>
                <div class="sync-kv">
                  <span class="k">Connection</span>
                  <span class="v">
                    <ResourceLink
                      href={resourceHref({ kind: "connection", process: r.process, connection: r.connection })}
                      active={isActivePath(selectedPath, resourceHref({ kind: "connection", process: r.process, connection: r.connection }))}
                      kind="connection"
                    >
                      {r.connection}
                    </ResourceLink>
                  </span>
                </div>
              </div>

              <div class="sync-instance-foot" style="display: block">
                <div class="sync-kv" style="margin-bottom: 8px">
                  <span class="k">Task</span>
                  <span class="v">
                    {r.task_id != null ? (
                      <ResourceLink
                        href={resourceHref({ kind: "task", process: r.process, taskId: r.task_id })}
                        active={isActivePath(selectedPath, resourceHref({ kind: "task", process: r.process, taskId: r.task_id }))}
                        kind="task"
                      >
                        {r.task_name ?? "task"} (#{r.task_id})
                      </ResourceLink>
                    ) : (
                      <span class="muted">{"\u2014"}</span>
                    )}
                  </span>
                </div>
                <div style="margin-bottom: 8px">
                  <div class="k muted" style="font-size: 11px; margin-bottom: 4px">Context</div>
                  <RequestContextTree
                    r={r}
                    interactionsByTask={interactionsByTask}
                    selectedPath={selectedPath}
                  />
                </div>
                <div style="margin-bottom: 8px">
                  <div class="k muted" style="font-size: 11px; margin-bottom: 4px">Causal graph</div>
                  <RequestInlineGraph
                    r={r}
                    dump={dumpByProcess.get(r.process)}
                    interactionsByTask={interactionsByTask}
                  />
                </div>
                <div>
                  <div class="k muted" style="font-size: 11px; margin-bottom: 4px">Backtrace</div>
                  <Expandable content={r.backtrace} />
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div>
          {chains.map(([chainId, chainRequests]) => {
            const spanOwner = new Map<string, FlatRequest>();
            for (const req of chainRequests) {
              if (req.span_id && !spanOwner.has(req.span_id)) {
                spanOwner.set(req.span_id, req);
              }
            }

            const childrenByParent = new Map<string, FlatRequest[]>();
            const roots: FlatRequest[] = [];
            for (const req of chainRequests) {
              const parent =
                req.parent_span_id != null
                  ? spanOwner.get(req.parent_span_id)
                  : undefined;
              if (!parent) {
                roots.push(req);
                continue;
              }
              const parentKey = requestNodeKey(parent);
              const list = childrenByParent.get(parentKey) ?? [];
              list.push(req);
              childrenByParent.set(parentKey, list);
            }
            roots.sort((a, b) => b.elapsed_secs - a.elapsed_secs);

            return (
              <div class="card" key={chainId} style="margin-bottom: 12px">
                <div class="card-head">
                  <span class="mono text-purple">Chain</span>
                  <span class="mono" style="margin-left: 8px">{chainId}</span>
                  <span class="muted" style="margin-left: auto">
                    {chainRequests.length} request(s)
                  </span>
                </div>
                <div style="padding: 8px 10px 12px">
                  {roots.map((root) => (
                    <RequestTreeNode
                      key={requestNodeKey(root)}
                      node={root}
                      childrenByParent={childrenByParent}
                      interactionsByTask={interactionsByTask}
                      selectedPath={selectedPath}
                      depth={0}
                      seen={new Set<string>()}
                    />
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
