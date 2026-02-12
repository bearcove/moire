import { useState } from "preact/hooks";
import type { ProcessDump, TaskSnapshot } from "../types";
import { fmtAge, fmtDuration, classNames } from "../util";
import { Expandable } from "./Expandable";

interface Props {
  dumps: ProcessDump[];
  filter: string;
}

interface FlatTask extends TaskSnapshot {
  process: string;
  pid: number;
}

function stateClass(state: string): string {
  switch (state) {
    case "Polling":
      return "state-polling";
    case "Completed":
      return "state-completed";
    default:
      return "state-pending";
  }
}

function rowSeverity(t: FlatTask): string {
  if (t.state === "Polling") {
    const lastPoll = t.poll_events[t.poll_events.length - 1];
    if (lastPoll?.duration_secs != null && lastPoll.duration_secs > 1)
      return "severity-danger";
    if (lastPoll?.duration_secs != null && lastPoll.duration_secs > 0.1)
      return "severity-warn";
  }
  return "";
}

function matchesFilter(t: FlatTask, q: string): boolean {
  if (!q) return true;
  const lq = q.toLowerCase();
  return (
    t.name.toLowerCase().includes(lq) ||
    t.process.toLowerCase().includes(lq) ||
    t.state.toLowerCase().includes(lq) ||
    (t.parent_task_name?.toLowerCase().includes(lq) ?? false) ||
    String(t.id).includes(lq)
  );
}

export function TasksView({ dumps, filter }: Props) {
  const [view, setView] = useState<"table" | "tree">("table");

  const tasks: FlatTask[] = [];
  for (const d of dumps) {
    for (const t of d.tasks) {
      tasks.push({ ...t, process: d.process_name, pid: d.pid });
    }
  }

  const filtered = tasks.filter((t) => matchesFilter(t, filter));

  if (tasks.length === 0) {
    return (
      <div class="empty-state fade-in">
        <div class="icon">T</div>
        <p>No tasks tracked</p>
        <p class="sub">Tasks appear when using peeps::tasks::spawn_tracked()</p>
      </div>
    );
  }

  return (
    <div class="fade-in">
      <div style="margin-bottom: 12px; display: flex; gap: 8px">
        <button
          class={classNames("expand-trigger", view === "table" && "active")}
          style="padding: 4px 10px"
          onClick={() => setView("table")}
        >
          Table
        </button>
        <button
          class={classNames("expand-trigger", view === "tree" && "active")}
          style="padding: 4px 10px"
          onClick={() => setView("tree")}
        >
          Tree
        </button>
      </div>
      {view === "table" ? (
        <TaskTable tasks={filtered} />
      ) : (
        <TaskTree tasks={filtered} />
      )}
    </div>
  );
}

function TaskTable({ tasks }: { tasks: FlatTask[] }) {
  return (
    <div class="card">
      <table>
        <thead>
          <tr>
            <th>ID</th>
            <th>Process</th>
            <th>Name</th>
            <th>State</th>
            <th>Age</th>
            <th>Parent</th>
            <th>Polls</th>
            <th>Last Poll</th>
            <th>Backtrace</th>
          </tr>
        </thead>
        <tbody>
          {tasks.map((t) => (
            <tr key={`${t.pid}-${t.id}`} class={rowSeverity(t)}>
              <td class="mono">#{t.id}</td>
              <td class="mono">{t.process}</td>
              <td class="mono">{t.name}</td>
              <td>
                <span class={classNames("state-badge", stateClass(t.state))}>
                  {t.state}
                </span>
              </td>
              <td class="num">{fmtAge(t.age_secs)}</td>
              <td class="mono">
                {t.parent_task_id != null ? (
                  <span>
                    {t.parent_task_name ?? ""} (#{t.parent_task_id})
                  </span>
                ) : (
                  <span class="muted">{"\u2014"}</span>
                )}
              </td>
              <td class="num">{t.poll_events.length}</td>
              <td class="num">
                {t.poll_events.length > 0
                  ? fmtDuration(
                      t.poll_events[t.poll_events.length - 1].duration_secs ?? 0
                    )
                  : "\u2014"}
              </td>
              <td>
                <Expandable content={t.spawn_backtrace || null} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function TaskTree({ tasks }: { tasks: FlatTask[] }) {
  const byId = new Map<number, FlatTask>();
  for (const t of tasks) byId.set(t.id, t);

  const roots = tasks.filter((t) => t.parent_task_id == null || !byId.has(t.parent_task_id));
  const children = new Map<number, FlatTask[]>();
  for (const t of tasks) {
    if (t.parent_task_id != null && byId.has(t.parent_task_id)) {
      const list = children.get(t.parent_task_id) ?? [];
      list.push(t);
      children.set(t.parent_task_id, list);
    }
  }

  return (
    <div>
      {roots.map((t) => (
        <TreeNode key={t.id} task={t} children={children} depth={0} />
      ))}
    </div>
  );
}

function TreeNode({
  task: t,
  children,
  depth,
}: {
  task: FlatTask;
  children: Map<number, FlatTask[]>;
  depth: number;
}) {
  const kids = children.get(t.id) ?? [];
  return (
    <div class={depth === 0 ? "tree-node-root" : "tree-node"}>
      <div class="tree-item">
        <span class={classNames("state-badge", stateClass(t.state))} style="margin-right: 6px">
          {t.state}
        </span>
        <span class="mono">
          #{t.id} {t.name}
        </span>
        <span class="muted" style="margin-left: 8px">
          {t.process} &middot; {fmtAge(t.age_secs)}
        </span>
      </div>
      {kids.map((k) => (
        <TreeNode key={k.id} task={k} children={children} depth={depth + 1} />
      ))}
    </div>
  );
}
