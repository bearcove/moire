import ELK from "elkjs/lib/elk.bundled.js";
import type { ElkNode, ElkExtendedEdge, ElkEdgeSection } from "elkjs/lib/elk-api";
import { useEffect, useMemo, useState } from "preact/hooks";
import type { RelationshipIssue } from "../problems";
import { classNames } from "../util";

interface Props {
  issues: RelationshipIssue[];
}

type NodeKind = "blocked" | "owner";

interface GraphNode {
  id: string;
  key: string;
  label: string;
  kind: NodeKind;
  weight: number;
}

interface GraphEdge {
  id: string;
  source: string;
  target: string;
  severity: "danger" | "warn";
  count: number;
  worstTiming: number;
  worstTimingLabel: string;
  categories: Set<string>;
  processes: Set<string>;
}

interface LayoutNode {
  graph: GraphNode;
  x: number;
  y: number;
  width: number;
  height: number;
}

interface LayoutEdge {
  graph: GraphEdge;
  path: string;
  labelX: number;
  labelY: number;
}

interface LayoutData {
  width: number;
  height: number;
  nodes: LayoutNode[];
  edges: LayoutEdge[];
}

function truncate(s: string, n: number): string {
  if (s.length <= n) return s;
  return `${s.slice(0, n - 1)}…`;
}

function makeGraph(issues: RelationshipIssue[]): { nodes: GraphNode[]; edges: GraphEdge[] } {
  const ownerNodes = new Map<string, GraphNode>();
  const blockedNodes = new Map<string, GraphNode>();
  const edges = new Map<string, GraphEdge>();

  for (const issue of issues) {
    const ownerKey = issue.owner ?? issue.waitsOn;
    const blockedKey = `${issue.process}::${issue.blocked}`;

    if (!ownerNodes.has(ownerKey)) {
      ownerNodes.set(ownerKey, {
        id: `owner:${ownerNodes.size}`,
        key: ownerKey,
        label: ownerKey,
        kind: "owner",
        weight: 0,
      });
    }

    if (!blockedNodes.has(blockedKey)) {
      blockedNodes.set(blockedKey, {
        id: `blocked:${blockedNodes.size}`,
        key: blockedKey,
        label: blockedKey,
        kind: "blocked",
        weight: 0,
      });
    }

    const owner = ownerNodes.get(ownerKey)!;
    const blocked = blockedNodes.get(blockedKey)!;
    owner.weight += issue.count;
    blocked.weight += issue.count;

    const edgeKey = `${blocked.id}->${owner.id}`;
    const existing = edges.get(edgeKey);
    if (existing) {
      existing.count += issue.count;
      if (issue.severity === "danger") existing.severity = "danger";
      if (issue.timing > existing.worstTiming) {
        existing.worstTiming = issue.timing;
        existing.worstTimingLabel = issue.timingLabel;
      }
      existing.categories.add(issue.category);
      existing.processes.add(issue.process);
    } else {
      edges.set(edgeKey, {
        id: `edge:${edges.size}`,
        source: blocked.id,
        target: owner.id,
        severity: issue.severity,
        count: issue.count,
        worstTiming: issue.timing,
        worstTimingLabel: issue.timingLabel,
        categories: new Set([issue.category]),
        processes: new Set([issue.process]),
      });
    }
  }

  return {
    nodes: [...blockedNodes.values(), ...ownerNodes.values()],
    edges: [...edges.values()],
  };
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
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 - 6 };
}

export function CausalGraph({ issues }: Props) {
  const { nodes, edges } = useMemo(() => makeGraph(issues), [issues]);
  const [layout, setLayout] = useState<LayoutData | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function runLayout() {
      if (nodes.length === 0) {
        setLayout(null);
        return;
      }

      const elk = new ELK();
      const elkNodes: ElkNode[] = nodes.map((n) => ({
        id: n.id,
        width: Math.max(180, Math.min(320, 160 + n.label.length * 4)),
        height: 34,
      }));

      const elkEdges: ElkExtendedEdge[] = edges.map((e) => ({
        id: e.id,
        sources: [e.source],
        targets: [e.target],
      }));

      const graph: ElkNode = {
        id: "root",
        layoutOptions: {
          "elk.algorithm": "layered",
          "elk.direction": "RIGHT",
          "elk.edgeRouting": "SPLINES",
          "elk.padding": "[top=24,left=28,bottom=24,right=28]",
          "elk.spacing.nodeNode": "40",
          "elk.layered.spacing.nodeNodeBetweenLayers": "120",
          "elk.layered.nodePlacement.strategy": "NETWORK_SIMPLEX",
        },
        children: elkNodes,
        edges: elkEdges,
      };

      const out = await elk.layout(graph);
      if (cancelled) return;

      const nodeById = new Map(nodes.map((n) => [n.id, n]));
      const edgeById = new Map(edges.map((e) => [e.id, e]));

      const laidNodes: LayoutNode[] = (out.children ?? []).map((n) => ({
        graph: nodeById.get(n.id!)!,
        x: n.x ?? 0,
        y: n.y ?? 0,
        width: n.width ?? 180,
        height: n.height ?? 34,
      }));

      const laidEdges: LayoutEdge[] = [];
      for (const e of out.edges ?? []) {
        const graphEdge = edgeById.get(e.id!);
        if (!graphEdge) continue;
        const sec = (e.sections ?? [])[0];
        if (!sec) continue;
        const path = sectionPath(sec);
        if (!path) continue;
        const pos = edgeLabelPos(sec);
        laidEdges.push({ graph: graphEdge, path, labelX: pos.x, labelY: pos.y });
      }

      setLayout({
        width: out.width ?? 900,
        height: out.height ?? 280,
        nodes: laidNodes,
        edges: laidEdges,
      });
    }

    runLayout().catch((e) => {
      if (!cancelled) {
        console.error("ELK layout failed", e);
        setLayout(null);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [nodes, edges]);

  if (issues.length === 0) return null;

  const selectedNode = layout?.nodes.find((n) => n.graph.id === selectedNodeId) ?? null;
  const selectedEdge = layout?.edges.find((e) => e.graph.id === selectedEdgeId) ?? null;
  const nodeLabelById = new Map((layout?.nodes ?? []).map((n) => [n.graph.id, n.graph.label]));

  return (
    <div class="card">
      <div class="card-head">
        Dependency Graph (ELK)
        <span class="muted" style="margin-left: 8px; font-size: 11px; font-weight: 500">
          click node/edge to inspect
        </span>
      </div>

      <div class="causal-graph-wrap">
        {!layout && <div class="muted">Computing ELK layout…</div>}

        {layout && (
          <svg
            class="causal-graph"
            viewBox={`0 0 ${Math.max(900, layout.width)} ${Math.max(220, layout.height)}`}
            role="img"
          >
            <defs>
              <marker id="elk-arrow-danger" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto">
                <path d="M0,0 L10,3 L0,6 z" fill="var(--red)" />
              </marker>
              <marker id="elk-arrow-warn" markerWidth="10" markerHeight="10" refX="9" refY="3" orient="auto">
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
                    selectedEdgeId === e.graph.id && "selected",
                  )}
                  marker-end={e.graph.severity === "danger" ? "url(#elk-arrow-danger)" : "url(#elk-arrow-warn)"}
                />
                <path
                  d={e.path}
                  class="causal-edge-hit"
                  onClick={() => {
                    setSelectedEdgeId(e.graph.id);
                    setSelectedNodeId(null);
                  }}
                />
                <text x={e.labelX} y={e.labelY} class="causal-edge-label">
                  {e.graph.count > 1 ? `${e.graph.count}x ` : ""}
                  {e.graph.worstTimingLabel}
                </text>
              </g>
            ))}

            {layout.nodes.map((n) => (
              <g
                key={n.graph.id}
                onClick={() => {
                  setSelectedNodeId(n.graph.id);
                  setSelectedEdgeId(null);
                }}
              >
                <rect
                  x={n.x}
                  y={n.y}
                  width={n.width}
                  height={n.height}
                  rx="7"
                  class={classNames(
                    "causal-node",
                    n.graph.kind,
                    selectedNodeId === n.graph.id && "selected",
                  )}
                />
                <text x={n.x + 8} y={n.y + 21} class="causal-node-text">
                  {truncate(n.graph.label, 44)}
                </text>
              </g>
            ))}
          </svg>
        )}
      </div>

      {(selectedNode || selectedEdge) && (
        <div class="causal-details">
          {selectedNode && (
            <div>
              <span class="mono text-blue">node</span>
              <span class="mono" style="margin-left: 8px">{selectedNode.graph.label}</span>
              <span class="muted" style="margin-left: 10px">{selectedNode.graph.kind}</span>
              <span class="muted" style="margin-left: 10px">weight {selectedNode.graph.weight}</span>
            </div>
          )}

          {selectedEdge && (
            <div>
              <span class="mono text-purple">edge</span>
              <span class="mono" style="margin-left: 8px">
                {nodeLabelById.get(selectedEdge.graph.source) ?? selectedEdge.graph.source}
                {" \u2192 "}
                {nodeLabelById.get(selectedEdge.graph.target) ?? selectedEdge.graph.target}
              </span>
              <span class="muted" style="margin-left: 10px">{selectedEdge.graph.count} edges</span>
              <span class="muted" style="margin-left: 10px">worst {selectedEdge.graph.worstTimingLabel}</span>
              <span class="muted" style="margin-left: 10px">
                {Array.from(selectedEdge.graph.categories).join(", ")}
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
