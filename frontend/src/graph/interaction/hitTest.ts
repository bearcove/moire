import type { Point, GeometryNode, GeometryEdge } from "../geometry";

/** Returns the id of the first node whose worldRect contains worldPoint, or null. */
export function hitTestNodes(worldPoint: Point, nodes: GeometryNode[]): string | null {
  for (const node of nodes) {
    const { x, y, width, height } = node.worldRect;
    if (
      worldPoint.x >= x &&
      worldPoint.x <= x + width &&
      worldPoint.y >= y &&
      worldPoint.y <= y + height
    ) {
      return node.id;
    }
  }
  return null;
}

function pointToSegmentDistSq(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) {
    const ex = p.x - a.x;
    const ey = p.y - a.y;
    return ex * ex + ey * ey;
  }
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq));
  const cx = a.x + t * dx;
  const cy = a.y + t * dy;
  const fx = p.x - cx;
  const fy = p.y - cy;
  return fx * fx + fy * fy;
}

/**
 * Returns the id of the first edge whose polyline passes within threshold world-space
 * units of worldPoint, or null. Default threshold is 8px in world space.
 */
export function hitTestEdges(
  worldPoint: Point,
  edges: GeometryEdge[],
  threshold = 8,
): string | null {
  const threshSq = threshold * threshold;
  for (const edge of edges) {
    const pts = edge.polyline;
    for (let i = 0; i < pts.length - 1; i++) {
      if (pointToSegmentDistSq(worldPoint, pts[i], pts[i + 1]) <= threshSq) {
        return edge.id;
      }
    }
  }
  return null;
}
