import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import type { ElkInputEdge, ElkInputNode, ElkLayoutRequest, ElkLayoutResult, GraphNode } from "../types";
import ElkWorker from "../workers/elk-worker?worker";

// Mock graph data for prototype
function generateMockGraph(): ElkLayoutRequest {
  const kinds = ["request", "response", "task", "future", "lock"];
  const nodes: ElkInputNode[] = [];
  const edges: ElkInputEdge[] = [];

  for (let i = 0; i < 12; i++) {
    const kind = kinds[i % kinds.length];
    nodes.push({
      id: `node-${i}`,
      kind,
      label: `${kind}:${i}`,
      width: 140,
      height: 28,
    });
  }

  const connections = [
    [0, 2], [2, 3], [3, 4], [1, 5], [5, 6], [6, 7],
    [2, 8], [8, 9], [4, 10], [10, 11], [0, 1], [3, 7],
  ];
  for (const [s, t] of connections) {
    if (s < nodes.length && t < nodes.length) {
      edges.push({
        id: `e-${s}-${t}`,
        source: nodes[s].id,
        target: nodes[t].id,
        kind: "depends_on",
      });
    }
  }

  return { nodes, edges };
}

interface GraphViewProps {
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  hoveredNode: GraphNode | null;
  onHoverNode: (node: GraphNode | null) => void;
}

export function GraphView({ selectedNodeId, onSelectNode, hoveredNode, onHoverNode }: GraphViewProps) {
  const [layout, setLayout] = useState<ElkLayoutResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const workerRef = useRef<Worker | null>(null);
  const [offset, setOffset] = useState({ x: 20, y: 20 });

  useEffect(() => {
    const worker = new ElkWorker();
    workerRef.current = worker;

    worker.onmessage = (e: MessageEvent<ElkLayoutResult & { error?: string }>) => {
      if (e.data.error) {
        setError(e.data.error);
      } else {
        setLayout(e.data);
      }
    };

    const mock = generateMockGraph();
    worker.postMessage(mock);

    return () => worker.terminate();
  }, []);

  const nodeMap = new Map<string, GraphNode>();
  if (layout) {
    for (const n of layout.nodes) nodeMap.set(n.id, n);
  }

  const handleNodeClick = useCallback((nodeId: string) => {
    onSelectNode(nodeId);
  }, [onSelectNode]);

  if (error) {
    return (
      <div class="panel">
        <div class="panel-header">Graph</div>
        <div style={{ padding: 12 }} class="error-text">{error}</div>
      </div>
    );
  }

  if (!layout) {
    return (
      <div class="panel">
        <div class="panel-header">Graph</div>
        <div style={{ padding: 12, color: "light-dark(#6e6e73, #98989d)", fontSize: 12 }}>
          Computing layout...
        </div>
      </div>
    );
  }

  return (
    <div class="panel">
      <div class="panel-header">Graph (mock data)</div>
      <div class="graph-container" ref={containerRef}>
        <svg
          class="graph-canvas"
          viewBox={`0 0 ${layout.width + 40} ${layout.height + 40}`}
          preserveAspectRatio="xMidYMid meet"
        >
          <defs>
            <marker
              id="arrowhead"
              markerWidth="8"
              markerHeight="6"
              refX="8"
              refY="3"
              orient="auto"
            >
              <polygon
                points="0 0, 8 3, 0 6"
                fill="light-dark(#8e8e93, #636366)"
              />
            </marker>
          </defs>
          {layout.edges.map((edge, i) => {
            const src = nodeMap.get(edge.source);
            const tgt = nodeMap.get(edge.target);
            if (!src || !tgt) return null;
            const x1 = src.x + src.width / 2 + offset.x;
            const y1 = src.y + src.height + offset.y;
            const x2 = tgt.x + tgt.width / 2 + offset.x;
            const y2 = tgt.y + offset.y;
            return (
              <line
                key={`edge-${i}`}
                x1={x1} y1={y1}
                x2={x2} y2={y2}
                stroke="light-dark(#c7c7cc, #48484a)"
                stroke-width="1.5"
                marker-end="url(#arrowhead)"
              />
            );
          })}
        </svg>
        {layout.nodes.map((node) => (
          <div
            key={node.id}
            class={`graph-node graph-node--${node.kind}`}
            data-selected={node.id === selectedNodeId}
            style={{
              left: `${node.x + offset.x}px`,
              top: `${node.y + offset.y}px`,
              width: `${node.width}px`,
              height: `${node.height}px`,
              lineHeight: `${node.height - 8}px`,
            }}
            onClick={() => handleNodeClick(node.id)}
            onMouseEnter={() => onHoverNode(node)}
            onMouseLeave={() => onHoverNode(null)}
          >
            {node.label}
          </div>
        ))}
      </div>
      {hoveredNode && (
        <HoverCard node={hoveredNode} />
      )}
    </div>
  );
}

function HoverCard({ node }: { node: GraphNode }) {
  return (
    <div
      class="hover-card"
      style={{
        left: `${node.x + node.width + 30}px`,
        top: `${node.y + 60}px`,
        position: "absolute",
      }}
    >
      <dl>
        <dt>id</dt>
        <dd>{node.id}</dd>
        <dt>kind</dt>
        <dd>{node.kind}</dd>
        <dt>position</dt>
        <dd>({node.x.toFixed(0)}, {node.y.toFixed(0)})</dd>
      </dl>
    </div>
  );
}
