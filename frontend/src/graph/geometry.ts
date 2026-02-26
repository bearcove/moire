export type Point = { x: number; y: number };
export type Rect = { x: number; y: number; width: number; height: number };

export interface GeometryNode {
  id: string;
  kind: string; // "graphNode"
  worldRect: Rect;
  data: any; // GraphNodeData shape
}

export interface GeometryGroup {
  id: string;
  scopeKind: string; // "process" | "crate"
  label: string;
  worldRect: Rect;
  labelRect?: Rect;
  members: string[]; // entity node ids
  data: any; // scope color info etc
}

export interface GeometryEdge {
  id: string;
  sourceId: string;
  targetId: string;
  polyline: Point[]; // ordered bend points from ELK sections
  kind: string; // edge kind from EdgeDef
  data: any; // edge metadata: style, tooltip, label, markerSize, etc.
}

export interface GraphGeometry {
  nodes: GeometryNode[];
  groups: GeometryGroup[];
  edges: GeometryEdge[];
  bounds: Rect; // bounding box of all geometry
  portAnchors: Map<string, Point>; // port id → face-snapped world position
}
