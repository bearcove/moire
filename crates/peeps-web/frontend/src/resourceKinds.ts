const RESOURCE_KINDS = new Set<string>(["connection", "joinset", "task"]);

export function isResourceKind(kind: string): boolean {
  // Add future resource kinds here explicitly as they are introduced.
  return RESOURCE_KINDS.has(kind);
}
