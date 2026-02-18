import "./ScopeGroupNode.css";

export type ScopeGroupNodeData = {
  isScopeGroup: true;
  label: string;
  count: number;
  selected: boolean;
};

export function ScopeGroupNode({ data }: { data: ScopeGroupNodeData }) {
  return (
    <div className="scope-group">
      <div className="scope-group-header">
        <span className="scope-group-label">{data.label}</span>
        <span className="scope-group-meta">{data.count}</span>
      </div>
    </div>
  );
}
