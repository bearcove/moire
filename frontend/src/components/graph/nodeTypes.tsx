import { GraphNode } from "./GraphNode";
import { ChannelPairNode } from "./ChannelPairNode";
import { RpcPairNode } from "./RpcPairNode";
import { ScopeGroupNode } from "./ScopeGroupNode";
import { ElkRoutedEdge } from "./ElkRoutedEdge";
import type { RenderNodeForMeasure } from "../../layout";

export const nodeTypes = {
  graphNode: GraphNode,
  channelPairNode: ChannelPairNode,
  rpcPairNode: RpcPairNode,
  scopeGroupNode: ScopeGroupNode,
};

export const edgeTypes = { elkrouted: ElkRoutedEdge };

export const renderNodeForMeasure: RenderNodeForMeasure = (def) => {
  if (def.channelPair) {
    return (
      <ChannelPairNode
        data={{
          tx: def.channelPair.tx,
          rx: def.channelPair.rx,
          channelName: def.name,
          selected: false,
          statTone: def.statTone,
          measureMode: true,
        }}
      />
    );
  }
  if (def.rpcPair) {
    return (
      <RpcPairNode
        data={{
          req: def.rpcPair.req,
          resp: def.rpcPair.resp,
          rpcName: def.name,
          selected: false,
          measureMode: true,
        }}
      />
    );
  }
  return (
    <GraphNode
      data={{
        kind: def.kind,
        label: def.name,
        inCycle: def.inCycle,
        selected: false,
        status: def.status,
        ageMs: def.ageMs,
        stat: def.stat,
        statTone: def.statTone,
        measureMode: true,
      }}
    />
  );
};
