import { describe, expect, it } from "vitest";
import type { SnapshotCutResponse } from "./api/types.generated";
import type { EdgeDef } from "./snapshot";
import { buildBacktraceIndex, collapseEdgesThroughHiddenNodes } from "./snapshot";

function edge(id: string, source: string, target: string): EdgeDef {
  return {
    id,
    source,
    target,
    kind: "waiting_on",
  };
}

describe("collapseEdgesThroughHiddenNodes", () => {
  it("keeps visible direct edges", () => {
    const edges = [edge("ab", "a", "b"), edge("bc", "b", "c")];
    const visible = new Set(["a", "b"]);

    const collapsed = collapseEdgesThroughHiddenNodes(edges, visible);

    expect(collapsed.map((e) => e.id)).toEqual(["ab"]);
  });

  it("synthesizes across hidden nodes in directed paths", () => {
    const edges = [edge("ah", "a", "h"), edge("hb", "h", "b")];
    const visible = new Set(["a", "b"]);

    const collapsed = collapseEdgesThroughHiddenNodes(edges, visible);

    expect(collapsed).toContainEqual(
      expect.objectContaining({
        id: "collapsed-a-b",
        source: "a",
        target: "b",
        kind: "polls",
      }),
    );
  });

  it("synthesizes when the hidden intermediary only has incoming edges from visible nodes", () => {
    const edges = [edge("ha", "h", "a"), edge("hb", "h", "b")];
    const visible = new Set(["a", "b"]);

    const collapsed = collapseEdgesThroughHiddenNodes(edges, visible);

    expect(collapsed).toContainEqual(
      expect.objectContaining({
        id: "collapsed-a-b",
        source: "a",
        target: "b",
        kind: "polls",
      }),
    );
  });
});

describe("buildBacktraceIndex", () => {
  // f[verify display.backtrace.catalog]
  it("resolves frame_ids through catalog and rejects missing frame references", () => {
    const snapshot: SnapshotCutResponse = {
      snapshot_id: 1,
      captured_at_unix_ms: 0,
      processes: [],
      timed_out_processes: [],
      frames: [
        { frame_id: 11, frame: { unresolved: { module_path: "/bin/app", rel_pc: 16, reason: "symbolication pending" } } },
      ],
      backtraces: [{ backtrace_id: 101, frame_ids: [11] }],
    };

    const index = buildBacktraceIndex(snapshot);
    expect(index.get(101)?.frame_ids).toEqual([11]);
    expect(index.get(101)?.frames).toHaveLength(1);

    const broken: SnapshotCutResponse = {
      ...snapshot,
      backtraces: [{ backtrace_id: 102, frame_ids: [99] }],
    };
    expect(() => buildBacktraceIndex(broken)).toThrow("references missing frame id 99");
  });
});
