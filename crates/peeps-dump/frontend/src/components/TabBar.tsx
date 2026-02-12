import type { Tab } from "../App";
import type { ProcessDump } from "../types";
import { classNames } from "../util";

interface TabBarProps {
  tabs: readonly Tab[];
  active: Tab;
  onSelect: (t: Tab) => void;
  dumps: ProcessDump[];
}

function badgeCount(tab: Tab, dumps: ProcessDump[]): number | null {
  switch (tab) {
    case "tasks":
      return dumps.reduce((s, d) => s + d.tasks.length, 0);
    case "threads":
      return dumps.reduce((s, d) => s + d.threads.length, 0);
    case "locks":
      return dumps.reduce(
        (s, d) => s + (d.locks?.locks.length ?? 0),
        0
      );
    case "sync": {
      let n = 0;
      for (const d of dumps) {
        if (!d.sync) continue;
        n +=
          d.sync.mpsc_channels.length +
          d.sync.oneshot_channels.length +
          d.sync.watch_channels.length +
          d.sync.once_cells.length;
      }
      return n;
    }
    case "requests":
      return dumps.reduce(
        (s, d) =>
          s +
          (d.roam?.connections.reduce(
            (s2, c) => s2 + c.in_flight.length,
            0
          ) ?? 0),
        0
      );
    case "connections":
      return dumps.reduce(
        (s, d) => s + (d.roam?.connections.length ?? 0),
        0
      );
    case "processes":
      return dumps.length;
    case "shm":
      return null;
  }
}

const TAB_LABELS: Record<Tab, string> = {
  tasks: "Tasks",
  threads: "Threads",
  locks: "Locks",
  sync: "Sync",
  requests: "Requests",
  connections: "Connections",
  processes: "Processes",
  shm: "SHM",
};

export function TabBar({ tabs, active, onSelect, dumps }: TabBarProps) {
  return (
    <div class="tab-bar">
      {tabs.map((t) => {
        const count = badgeCount(t, dumps);
        return (
          <div
            key={t}
            class={classNames("tab", t === active && "active")}
            onClick={() => onSelect(t)}
          >
            {TAB_LABELS[t]}
            {count != null && <span class="tab-badge">{count}</span>}
          </div>
        );
      })}
    </div>
  );
}
