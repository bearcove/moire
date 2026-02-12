import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import type { ProcessDump } from "./types";
import { fetchDumps } from "./api";
import { Header } from "./components/Header";
import { TabBar } from "./components/TabBar";
import { TasksView } from "./components/TasksView";
import { ThreadsView } from "./components/ThreadsView";
import { LocksView } from "./components/LocksView";
import { SyncView } from "./components/SyncView";
import { ProcessesView } from "./components/ProcessesView";
import { ConnectionsView } from "./components/ConnectionsView";
import { RequestsView } from "./components/RequestsView";
import { ShmView } from "./components/ShmView";

import "./styles.css";

const TABS = [
  "tasks",
  "threads",
  "locks",
  "sync",
  "requests",
  "connections",
  "processes",
  "shm",
] as const;
export type Tab = (typeof TABS)[number];

export function App() {
  const [dumps, setDumps] = useState<ProcessDump[]>([]);
  const [tab, setTab] = useState<Tab>("tasks");
  const [filter, setFilter] = useState("");
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await fetchDumps();
      setDumps(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    intervalRef.current = setInterval(refresh, 2000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [refresh]);

  const hasSync = dumps.some((d) => d.sync != null);
  const hasRoam = dumps.some((d) => d.roam != null);
  const hasShm = dumps.some((d) => d.shm != null);
  const hasLocks = dumps.some((d) => d.locks != null);

  const visibleTabs = TABS.filter((t) => {
    if (t === "sync" && !hasSync) return false;
    if (t === "requests" && !hasRoam) return false;
    if (t === "connections" && !hasRoam) return false;
    if (t === "shm" && !hasShm) return false;
    if (t === "locks" && !hasLocks) return false;
    return true;
  });

  return (
    <div class="app">
      <Header dumps={dumps} filter={filter} onFilter={setFilter} onRefresh={refresh} error={error} />
      <TabBar tabs={visibleTabs} active={tab} onSelect={setTab} dumps={dumps} />
      <div class="content">
        {tab === "tasks" && <TasksView dumps={dumps} filter={filter} />}
        {tab === "threads" && <ThreadsView dumps={dumps} filter={filter} />}
        {tab === "locks" && <LocksView dumps={dumps} filter={filter} />}
        {tab === "sync" && <SyncView dumps={dumps} filter={filter} />}
        {tab === "requests" && <RequestsView dumps={dumps} filter={filter} />}
        {tab === "connections" && (
          <ConnectionsView dumps={dumps} filter={filter} />
        )}
        {tab === "processes" && (
          <ProcessesView dumps={dumps} filter={filter} />
        )}
        {tab === "shm" && <ShmView dumps={dumps} filter={filter} />}
      </div>
    </div>
  );
}
