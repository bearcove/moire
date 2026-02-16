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
    const methodPos = html.indexOf(">Method<");
    const corrPos = html.indexOf(">Correlation<");
    const processPos = html.indexOf(">Process<");
    const sourcePos = html.indexOf(">Source<");
    expect(idPos).toBeGreaterThan(-1);
    expect(methodPos).toBeGreaterThan(idPos);
    expect(corrPos).toBeGreaterThan(methodPos);
    expect(processPos).toBeGreaterThan(corrPos);
    expect(sourcePos).toBeGreaterThan(processPos);
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
});

describe("inspector integration", () => {
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
