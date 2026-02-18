import type { EdgeProps } from "@xyflow/react";
import type { ElkPoint } from "../../layout";
import "./ElkRoutedEdge.css";

export function ElkRoutedEdge({ id, data, style, markerEnd, selected }: EdgeProps) {
  const edgeData = data as
    | {
        points?: ElkPoint[];
        tooltip?: string;
        ghost?: boolean;
        edgeLabel?: string;
        edgePending?: boolean;
      }
    | undefined;
  const points = edgeData?.points ?? [];
  const ghost = edgeData?.ghost ?? false;
  const edgeLabel = edgeData?.edgeLabel;
  const edgePending = edgeData?.edgePending ?? false;
  if (points.length < 2) return null;

  const [start, ...rest] = points;
  let d = `M ${start.x} ${start.y}`;
  if (rest.length === 1) {
    d += ` L ${rest[0].x} ${rest[0].y}`;
  } else {
    for (let i = 0; i < rest.length - 1; i++) {
      const curr = rest[i];
      const next = rest[i + 1];
      if (i < rest.length - 2) {
        const midX = (curr.x + next.x) / 2;
        const midY = (curr.y + next.y) / 2;
        d += ` Q ${curr.x} ${curr.y}, ${midX} ${midY}`;
      } else {
        d += ` Q ${curr.x} ${curr.y}, ${next.x} ${next.y}`;
      }
    }
  }

  const labelAnchor: { x: number; y: number; dx: number; dy: number } = (() => {
    if (points.length < 2) {
      const p = points[0] ?? { x: 0, y: 0 };
      return { x: p.x, y: p.y, dx: 0, dy: 1 };
    }
    let totalLength = 0;
    for (let i = 1; i < points.length; i++) {
      const dx = points[i].x - points[i - 1].x;
      const dy = points[i].y - points[i - 1].y;
      totalLength += Math.hypot(dx, dy);
    }
    if (totalLength <= 0) {
      const p = points[0];
      return { x: p.x, y: p.y, dx: 0, dy: 1 };
    }
    const halfway = totalLength / 2;
    let traversed = 0;
    for (let i = 1; i < points.length; i++) {
      const start = points[i - 1];
      const end = points[i];
      const segLength = Math.hypot(end.x - start.x, end.y - start.y);
      if (traversed + segLength >= halfway) {
        const remain = halfway - traversed;
        const t = segLength <= 0 ? 0 : remain / segLength;
        const dx = end.x - start.x;
        const dy = end.y - start.y;
        return {
          x: start.x + (end.x - start.x) * t,
          y: start.y + (end.y - start.y) * t,
          dx,
          dy,
        };
      }
      traversed += segLength;
    }
    const mid = points[Math.floor(points.length / 2)];
    return { x: mid.x, y: mid.y, dx: 0, dy: 1 };
  })();
  const dirLen = Math.hypot(labelAnchor.dx, labelAnchor.dy) || 1;
  const nx = -labelAnchor.dy / dirLen;
  const ny = labelAnchor.dx / dirLen;
  const labelX = labelAnchor.x + nx * 20;
  const labelY = labelAnchor.y + ny * 20;

  return (
    <g style={ghost ? { opacity: 0.2, pointerEvents: "none" } : undefined}>
      <path
        d={d}
        fill="none"
        stroke="transparent"
        strokeWidth={14}
        style={{ cursor: "pointer", pointerEvents: ghost ? "none" : "all" }}
      />
      {selected && (
        <>
          <path
            d={d}
            fill="none"
            stroke="var(--accent, #3b82f6)"
            strokeWidth={10}
            strokeLinecap="round"
            opacity={0.18}
            className="edge-glow"
          />
          <path
            d={d}
            fill="none"
            stroke="var(--accent, #3b82f6)"
            strokeWidth={5}
            strokeLinecap="round"
            opacity={0.45}
          />
        </>
      )}
      <path
        id={id}
        d={d}
        style={{
          ...(style as React.CSSProperties),
          ...(selected ? { stroke: "var(--accent, #3b82f6)", strokeWidth: 2.5 } : {}),
        }}
        markerEnd={markerEnd as string}
        fill="none"
        className="react-flow__edge-path"
      />
      {edgeLabel && (
        <g className="edge-label" transform={`translate(${labelX}, ${labelY})`}>
          <text className="edge-label-text" textAnchor="middle" dominantBaseline="middle">
            {edgeLabel}
            {edgePending && <tspan className="edge-label-symbol"> ⏳</tspan>}
          </text>
        </g>
      )}
    </g>
  );
}
