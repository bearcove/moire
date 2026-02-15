import type { JumpNowResponse } from "../types";

interface HeaderProps {
  snapshot: JumpNowResponse | null;
  loading: boolean;
  onJumpNow: () => void;
}

export function Header({ snapshot, loading, onJumpNow }: HeaderProps) {
  return (
    <div class="header">
      <span class="header-title">peeps</span>
      <span class={`snapshot-badge ${snapshot ? "snapshot-badge--active" : ""}`}>
        {snapshot ? `snapshot #${snapshot.snapshot_id}` : "no snapshot"}
      </span>
      {snapshot && (
        <span class="snapshot-badge">
          {snapshot.responded}/{snapshot.requested} responded
          {snapshot.timed_out > 0 && `, ${snapshot.timed_out} timed out`}
        </span>
      )}
      <span class="header-spacer" />
      <button
        class={`btn btn--primary ${loading ? "btn--loading" : ""}`}
        onClick={onJumpNow}
        disabled={loading}
      >
        {loading ? "Jumping..." : "Jump to now"}
      </button>
    </div>
  );
}
