import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Inspector } from "./Inspector";
import {
  CommonInspectorFields,
  formatRelativeTimestampFromOrigin,
  getCorrelation,
  getCreatedAtNs,
  getMethod,
  getSource,
  resolveTimelineOriginNs,
} from "./inspectorShared";
import type { SnapshotNode } from "../types";

describe("CommonInspectorFields", () => {
  it("renders canonical fields in order and omits missing optional rows", () => {
    const html = renderToStaticMarkup(
      <CommonInspectorFields
        id="request:abc"
        process="api"
        attrs={{
          method: "GetUser",
          correlation: "corr-1",
          source: "/srv/app/src/handler.rs:19",
        }}
      />,
    );

    const idPos = html.indexOf(">ID<");
    const processPos = html.indexOf(">Process<");
    const methodPos = html.indexOf(">Method<");
    const corrPos = html.indexOf(">Correlation<");
    const sourcePos = html.indexOf(">Source<");
    expect(idPos).toBeGreaterThan(-1);
    expect(processPos).toBeGreaterThan(idPos);
    expect(methodPos).toBeGreaterThan(processPos);
    expect(corrPos).toBeGreaterThan(methodPos);
    expect(sourcePos).toBeGreaterThan(corrPos);
  });
});

describe("canonical extractors", () => {
  it("reads only canonical keys", () => {
    const attrs = {
      method: "CanonicalMethod",
      correlation: "canonical-corr",
      source: "/tmp/work.rs:1",
      "request.method": "LegacyMethod",
      "ctx.location": "/legacy/path.rs:2",
    };
    expect(getMethod(attrs)).toBe("CanonicalMethod");
    expect(getCorrelation(attrs)).toBe("canonical-corr");
    expect(getSource(attrs)).toBe("/tmp/work.rs:1");
    expect(getMethod({ "request.method": "LegacyOnly" })).toBeUndefined();
    expect(getCorrelation({ correlation_key: "legacy-only" })).toBeUndefined();
    expect(getSource({ "ctx.location": "/legacy/path.rs:2" })).toBeUndefined();
    expect(getCreatedAtNs({ created_at_ns: 123 })).toBeUndefined();
  });

  it("normalizes created_at units to ns", () => {
    expect(getCreatedAtNs({ created_at: 1_700_000_000 })).toBe(1_700_000_000_000_000_000);
    expect(getCreatedAtNs({ created_at: 1_700_000_000_000 })).toBe(1_700_000_000_000_000_000);
    expect(getCreatedAtNs({ created_at: 1_700_000_000_000_000 })).toBe(1_700_000_000_000_000_000);
    expect(getCreatedAtNs({ created_at: 1_700_000_000_000_000_000 })).toBe(
      1_700_000_000_000_000_000,
    );
  });
});

describe("timeline origin resolver", () => {
  it("falls back to first event for insane created_at and keeps sane created_at", () => {
    const firstEvent = 1_700_000_000_000_000_000;
    expect(resolveTimelineOriginNs({ created_at: firstEvent + 10 }, firstEvent)).toBe(firstEvent);
    expect(resolveTimelineOriginNs({ created_at: firstEvent - 31 * 24 * 60 * 60 * 1_000_000_000 }, firstEvent)).toBe(
      firstEvent,
    );
    expect(resolveTimelineOriginNs({ created_at: firstEvent - 1_000_000 }, firstEvent)).toBe(
      firstEvent - 1_000_000,
    );
  });

  it("formats relative timestamps from canonical origin", () => {
    expect(formatRelativeTimestampFromOrigin(2_000_000_000, 1_000_000_000)).toBe("+1.000s");
  });

  it("uses canonical created_at as timeline origin for relative output", () => {
    const attrs = { created_at: 1_700_000_000_000_000_000 };
    const firstEvent = 1_700_000_000_500_000_000;
    const origin = resolveTimelineOriginNs(attrs, firstEvent);
    expect(origin).toBe(1_700_000_000_000_000_000);
    expect(formatRelativeTimestampFromOrigin(firstEvent, origin)).toBe("+500ms");
  });
});

describe("inspector integration", () => {
  it("renders one shared common-fields block for representative non-ghost kinds", () => {
    const sampleNodes: SnapshotNode[] = [
      {
        id: "request:1",
        kind: "request",
        process: "api",
        proc_key: "api-1",
        attrs: {
          created_at: 1_700_000_000_000_000_000,
          source: "/srv/api/request.rs:10",
          method: "GetUser",
          correlation: "corr-1",
        },
      },
      {
        id: "response:1",
        kind: "response",
        process: "api",
        proc_key: "api-1",
        attrs: {
          created_at: 1_700_000_000_100_000_000,
          source: "/srv/api/response.rs:44",
          method: "GetUser",
          correlation: "corr-1",
        },
      },
      {
        id: "tx:1",
        kind: "tx",
        process: "api",
        proc_key: "api-1",
        attrs: {
          created_at: 1_700_000_000_200_000_000,
          source: "/srv/api/channel.rs:21",
        },
      },
    ];

    for (const node of sampleNodes) {
      const html = renderToStaticMarkup(
        <Inspector
          snapshotId={1}
          snapshotCapturedAtNs={1_700_000_000_000_100_000}
          selectedRequest={null}
          selectedNode={node}
          selectedEdge={null}
          graph={{ nodes: [node], edges: [], ghostNodes: [] }}
          filteredNodeId={null}
          onFocusNode={() => {}}
          onSelectNode={() => {}}
          collapsed={false}
          onToggleCollapse={() => {}}
        />,
      );
      expect(html.match(/data-testid=\"common-fields\"/g)?.length ?? 0).toBe(1);
      const idPos = html.indexOf(">ID<");
      const processPos = html.indexOf(">Process<");
      const methodPos = html.indexOf(">Method<");
      const corrPos = html.indexOf(">Correlation<");
      const sourcePos = html.indexOf(">Source<");
      expect(idPos).toBeGreaterThan(-1);
      expect(processPos).toBeGreaterThan(idPos);
      if (methodPos !== -1) expect(methodPos).toBeGreaterThan(processPos);
      if (corrPos !== -1) {
        expect(corrPos).toBeGreaterThan(methodPos !== -1 ? methodPos : processPos);
      }
      const latestCommonPos =
        corrPos !== -1 ? corrPos : methodPos !== -1 ? methodPos : processPos;
      expect(sourcePos).toBeGreaterThan(latestCommonPos);
    }
  });

  it("fails inspector path when canonical created_at or source is missing", () => {
    const missingCreatedAt: SnapshotNode = {
      id: "response:no-created-at",
      kind: "response",
      process: "api",
      proc_key: "api-1",
      attrs: {
        source: "/srv/api/resp.rs:42",
      },
    };
    const missingSource: SnapshotNode = {
      id: "response:no-source",
      kind: "response",
      process: "api",
      proc_key: "api-1",
      attrs: {
        created_at: 1_700_000_000_000_000_000,
      },
    };

    const render = (node: SnapshotNode) =>
      renderToStaticMarkup(
        <Inspector
          snapshotId={1}
          snapshotCapturedAtNs={1_700_000_000_000_100_000}
          selectedRequest={null}
          selectedNode={node}
          selectedEdge={null}
          graph={{ nodes: [node], edges: [], ghostNodes: [] }}
          filteredNodeId={null}
          onFocusNode={() => {}}
          onSelectNode={() => {}}
          collapsed={false}
          onToggleCollapse={() => {}}
        />,
      );

    expect(() => render(missingCreatedAt)).toThrow(/requires created_at and source/);
    expect(() => render(missingSource)).toThrow(/requires created_at and source/);
  });

  it("ignores legacy method keys in shared common block", () => {
    const node: SnapshotNode = {
      id: "request:legacy",
      kind: "request",
      process: "api",
      proc_key: "api-1",
      attrs: {
        "request.method": "LegacyMethod",
        created_at: 1_700_000_000_000_000_000,
        source: "/srv/api/req.rs:12",
      },
    };

    const html = renderToStaticMarkup(
      <Inspector
        snapshotId={1}
        snapshotCapturedAtNs={1_700_000_000_000_100_000}
        selectedRequest={null}
        selectedNode={node}
        selectedEdge={null}
        graph={{ nodes: [node], edges: [], ghostNodes: [] }}
        filteredNodeId={null}
        onFocusNode={() => {}}
        onSelectNode={() => {}}
        collapsed={false}
        onToggleCollapse={() => {}}
      />,
    );

    expect(html.includes(">Method<")).toBe(false);
    expect(html.includes("LegacyMethod")).toBe(false);
  });

  it("renders exactly one Source row for response/rx canonical source", () => {
    const responseNode: SnapshotNode = {
      id: "response:1",
      kind: "response",
      process: "api",
      proc_key: "api-1",
      attrs: {
        source: "/srv/api/resp.rs:42",
        method: "GetUser",
        correlation: "corr-9",
        created_at: 1_700_000_000_000_000_000,
      },
    };

    const html = renderToStaticMarkup(
      <Inspector
        snapshotId={1}
        snapshotCapturedAtNs={1_700_000_000_000_100_000}
        selectedRequest={null}
        selectedNode={responseNode}
        selectedEdge={null}
        graph={{ nodes: [responseNode], edges: [], ghostNodes: [] }}
        filteredNodeId={null}
        onFocusNode={() => {}}
        onSelectNode={() => {}}
        collapsed={false}
        onToggleCollapse={() => {}}
      />,
    );

    expect(html.match(/>Source</g)?.length ?? 0).toBe(1);
  });
});
