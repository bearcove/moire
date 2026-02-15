import type { ProcessDump, MpscChannelSnapshot, OneshotChannelSnapshot, WatchChannelSnapshot, OnceCellSnapshot, SemaphoreSnapshot, RoamChannelSnapshot } from "../types";
import { fmtAge, fmtDuration, classNames } from "../util";
import { ResourceLink } from "./ResourceLink";
import { isActivePath, resourceHref } from "../routes";

interface Props {
  dumps: ProcessDump[];
  filter: string;
  selectedPath: string;
  mode?: "all" | "locks" | "channels";
}

type Severity = "danger" | "warn" | "idle";
type MpscBucket = "active" | "closed_benign" | "closed_waiters" | "idle";

function stateClass(state: string): string {
  switch (state) {
    case "Pending":
      return "state-pending";
    case "Sent":
    case "Received":
    case "Initialized":
      return "state-completed";
    case "Initializing":
      return "state-initializing";
    case "SenderDropped":
    case "ReceiverDropped":
      return "state-dropped";
    case "Empty":
      return "state-empty";
    default:
      return "";
  }
}

function severityRank(s: Severity): number {
  if (s === "danger") return 2;
  if (s === "warn") return 1;
  return 0;
}

function severityBadge(s: Severity) {
  if (s === "danger") return <span class="state-badge state-dropped">danger</span>;
  if (s === "warn") return <span class="state-badge state-pending">warn</span>;
  return <span class="state-badge state-empty">idle</span>;
}

function rowClass(s: Severity): string {
  if (s === "danger") return "severity-danger";
  if (s === "warn") return "severity-warn";
  return "";
}

function taskRef(process: string, id: number | null, name: string | null, selectedPath: string) {
  if (id == null) return <span class="muted">{"\u2014"}</span>;
  const href = resourceHref({ kind: "task", process, taskId: id });
  return (
    <ResourceLink href={href} active={isActivePath(selectedPath, href)} kind="task">
      {name ?? ""} (#{id})
    </ResourceLink>
  );
}

function processRef(process: string, selectedPath: string) {
  const href = resourceHref({ kind: "process", process });
  return (
    <ResourceLink href={href} active={isActivePath(selectedPath, href)} kind="process">
      {process}
    </ResourceLink>
  );
}

function mpscSeverity(ch: MpscChannelSnapshot): Severity {
  if (ch.send_waiters > 0) return "danger";
  if (ch.receiver_closed && ch.sender_count > 0) return "warn";
  if (ch.sender_closed || ch.receiver_closed) return "idle";
  return "idle";
}

function mpscBucket(ch: MpscChannelSnapshot): MpscBucket {
  if ((ch.sender_closed || ch.receiver_closed) && ch.send_waiters > 0) return "closed_waiters";
  if (ch.sender_closed || ch.receiver_closed) return "closed_benign";
  if (ch.send_waiters === 0 && ch.sent === ch.received) return "idle";
  return "active";
}

function mpscBacklog(ch: MpscChannelSnapshot): number {
  return Math.max(0, ch.sent - ch.received);
}

function mpscStateLabel(ch: MpscChannelSnapshot): string {
  if (ch.sender_closed && ch.receiver_closed) return "Both ends closed";
  if (ch.receiver_closed) return "Receiver closed";
  if (ch.sender_closed) return "Sender closed";
  if (ch.send_waiters > 0) return "Senders waiting";
  if (mpscBacklog(ch) > 0) return "Backlog growing";
  return "Healthy";
}

function mpscImpactScore(ch: MpscChannelSnapshot): number {
  let score = 0;
  if (ch.send_waiters > 0) score += 1000 + ch.send_waiters * 100;
  if (ch.receiver_closed && ch.sender_count > 0) score += 300;
  if (ch.sender_closed && !ch.receiver_closed) score += 40;
  score += Math.min(mpscBacklog(ch), 200);
  score += Math.floor(Math.min(ch.age_secs, 600) / 30);
  return score;
}

function oneshotSeverity(ch: OneshotChannelSnapshot): Severity {
  if (ch.state === "SenderDropped" || ch.state === "ReceiverDropped") return "danger";
  if (ch.state === "Pending" && ch.age_secs > 10) return "warn";
  return "idle";
}

function watchSeverity(ch: WatchChannelSnapshot): Severity {
  if (ch.receiver_count === 0 && ch.age_secs > 30) return "warn";
  return "idle";
}

function onceSeverity(ch: OnceCellSnapshot): Severity {
  if (ch.state === "Initializing" && ch.age_secs > 5) return "warn";
  return "idle";
}

function semaphoreSeverity(sem: SemaphoreSnapshot): Severity {
  if (sem.oldest_wait_secs > 10) return "danger";
  if (sem.oldest_wait_secs > 1) return "warn";
  if (sem.waiters > 0 && sem.permits_available === 0) return "warn";
  return "idle";
}

function semaphoreReason(sem: SemaphoreSnapshot): string {
  if (sem.oldest_wait_secs > 10) return `oldest waiter blocked ${fmtDuration(sem.oldest_wait_secs)}`;
  if (sem.oldest_wait_secs > 1) return `waiter blocked ${fmtDuration(sem.oldest_wait_secs)}`;
  if (sem.waiters > 0 && sem.permits_available === 0) return `${sem.waiters} waiter(s), no permits`;
  return "healthy";
}

function roamChannelSeverity(ch: RoamChannelSnapshot): Severity {
  if (ch.closed) return "danger";
  if (ch.age_secs > 10 && (ch.queue_depth ?? 0) === 0) return "warn";
  return "idle";
}

function roamChannelReason(ch: RoamChannelSnapshot): string {
  if (ch.closed) return "closed";
  if (ch.age_secs > 10 && (ch.queue_depth ?? 0) === 0) return `stale (${fmtAge(ch.age_secs)} idle)`;
  return "healthy";
}

function mpscReason(ch: MpscChannelSnapshot): string {
  const closedEnds = [];
  if (ch.sender_closed) closedEnds.push("sender closed");
  if (ch.receiver_closed) closedEnds.push("receiver closed");
  if (closedEnds.length > 0) return closedEnds.join(", ");
  if (ch.send_waiters > 0) return `${ch.send_waiters} sender waiter(s)`;
  if (mpscBacklog(ch) > 0) return `backlog ${mpscBacklog(ch)}`;
  return "healthy";
}

function oneshotReason(ch: OneshotChannelSnapshot): string {
  if (ch.state === "SenderDropped") return "sender dropped before send";
  if (ch.state === "ReceiverDropped") return "receiver dropped";
  if (ch.state === "Pending" && ch.age_secs > 10) return `pending for ${fmtAge(ch.age_secs)}`;
  return "healthy";
}

function watchReason(ch: WatchChannelSnapshot): string {
  if (ch.receiver_count === 0 && ch.age_secs > 30) return `no receivers for ${fmtAge(ch.age_secs)}`;
  return "healthy";
}

function onceReason(ch: OnceCellSnapshot): string {
  if (ch.state === "Initializing" && ch.age_secs > 5) return `initializing for ${fmtAge(ch.age_secs)}`;
  return "healthy";
}

export function SyncView({ dumps, filter, selectedPath, mode = "all" }: Props) {
  const q = filter.toLowerCase();

  const mpsc: { process: string; ch: MpscChannelSnapshot; severity: Severity; bucket: MpscBucket; impact: number }[] = [];
  const oneshot: { process: string; ch: OneshotChannelSnapshot; severity: Severity }[] = [];
  const watch: { process: string; ch: WatchChannelSnapshot; severity: Severity }[] = [];
  const once: { process: string; ch: OnceCellSnapshot; severity: Severity }[] = [];
  const sems: { process: string; sem: SemaphoreSnapshot; severity: Severity }[] = [];
  const roamChs: { process: string; ch: RoamChannelSnapshot; severity: Severity }[] = [];

  for (const d of dumps) {
    if (d.sync) {
      for (const ch of d.sync.mpsc_channels) {
        mpsc.push({
          process: d.process_name,
          ch,
          severity: mpscSeverity(ch),
          bucket: mpscBucket(ch),
          impact: mpscImpactScore(ch),
        });
      }
      for (const ch of d.sync.oneshot_channels) oneshot.push({ process: d.process_name, ch, severity: oneshotSeverity(ch) });
      for (const ch of d.sync.watch_channels) watch.push({ process: d.process_name, ch, severity: watchSeverity(ch) });
      for (const ch of d.sync.once_cells) once.push({ process: d.process_name, ch, severity: onceSeverity(ch) });
      for (const sem of d.sync.semaphores) sems.push({ process: d.process_name, sem, severity: semaphoreSeverity(sem) });
    }
    if (d.roam) {
      for (const ch of (d.roam.channel_details ?? [])) roamChs.push({ process: d.process_name, ch, severity: roamChannelSeverity(ch) });
    }
  }

  const filterMatch = (process: string, name: string) =>
    !q || process.toLowerCase().includes(q) || name.toLowerCase().includes(q);

  const semsFiltered = sems
    .filter((s) => filterMatch(s.process, s.sem.name))
    .sort((a, b) => {
      if (a.severity !== b.severity) return severityRank(b.severity) - severityRank(a.severity);
      if (a.sem.oldest_wait_secs !== b.sem.oldest_wait_secs) return b.sem.oldest_wait_secs - a.sem.oldest_wait_secs;
      if (a.sem.waiters !== b.sem.waiters) return b.sem.waiters - a.sem.waiters;
      return b.sem.age_secs - a.sem.age_secs;
    });

  const roamChsFiltered = roamChs
    .filter((r) => filterMatch(r.process, r.ch.name))
    .sort((a, b) => {
      if (a.severity !== b.severity) return severityRank(b.severity) - severityRank(a.severity);
      return b.ch.age_secs - a.ch.age_secs;
    });

  const mpscFiltered = mpsc
    .filter((m) => filterMatch(m.process, m.ch.name))
    .sort((a, b) => {
      if (a.impact !== b.impact) return b.impact - a.impact;
      if (a.severity !== b.severity) return severityRank(b.severity) - severityRank(a.severity);
      if (mpscBacklog(a.ch) !== mpscBacklog(b.ch)) return mpscBacklog(b.ch) - mpscBacklog(a.ch);
      return b.ch.age_secs - a.ch.age_secs;
    });

  const oneshotFiltered = oneshot
    .filter((o) => filterMatch(o.process, o.ch.name))
    .sort((a, b) => {
      if (a.severity !== b.severity) return severityRank(b.severity) - severityRank(a.severity);
      return b.ch.age_secs - a.ch.age_secs;
    });

  const watchFiltered = watch
    .filter((w) => filterMatch(w.process, w.ch.name))
    .sort((a, b) => {
      if (a.severity !== b.severity) return severityRank(b.severity) - severityRank(a.severity);
      if (a.ch.receiver_count !== b.ch.receiver_count) return a.ch.receiver_count - b.ch.receiver_count;
      return b.ch.age_secs - a.ch.age_secs;
    });

  const onceFiltered = once
    .filter((o) => filterMatch(o.process, o.ch.name))
    .sort((a, b) => {
      if (a.severity !== b.severity) return severityRank(b.severity) - severityRank(a.severity);
      return b.ch.age_secs - a.ch.age_secs;
    });

  const semsHot = semsFiltered.filter((s) => s.severity !== "idle");
  const semsIdle = semsFiltered.filter((s) => s.severity === "idle");

  const roamChsHot = roamChsFiltered.filter((r) => r.severity !== "idle");
  const roamChsIdle = roamChsFiltered.filter((r) => r.severity === "idle");

  const mpscSummary = {
    active: mpscFiltered.filter((m) => m.bucket === "active").length,
    closedBenign: mpscFiltered.filter((m) => m.bucket === "closed_benign").length,
    closedWithWaiters: mpscFiltered.filter((m) => m.bucket === "closed_waiters").length,
    idle: mpscFiltered.filter((m) => m.bucket === "idle").length,
  };

  const mpscGroups = Array.from(
    mpscFiltered.reduce((acc, m) => {
      const list = acc.get(m.ch.name) ?? [];
      list.push(m);
      acc.set(m.ch.name, list);
      return acc;
    }, new Map<string, typeof mpscFiltered>()),
  )
    .map(([name, items]) => {
      const processes = new Set(items.map((i) => i.process));
      items.sort((a, b) => b.impact - a.impact || b.ch.age_secs - a.ch.age_secs);
      return {
        name,
        items,
        processCount: processes.size,
        topImpact: items[0]?.impact ?? 0,
        topSeverity: items[0]?.severity ?? "idle",
      };
    })
    .sort((a, b) => {
      if (a.topImpact !== b.topImpact) return b.topImpact - a.topImpact;
      if (a.items.length !== b.items.length) return b.items.length - a.items.length;
      return a.name.localeCompare(b.name);
    });

  const oneshotHot = oneshotFiltered.filter((o) => o.severity !== "idle");
  const oneshotIdle = oneshotFiltered.filter((o) => o.severity === "idle");

  const watchHot = watchFiltered.filter((w) => w.severity !== "idle");
  const watchIdle = watchFiltered.filter((w) => w.severity === "idle");

  const onceHot = onceFiltered.filter((o) => o.severity !== "idle");
  const onceIdle = onceFiltered.filter((o) => o.severity === "idle");

  const showLocksFamily = mode === "all" || mode === "locks";
  const showChannelsFamily = mode === "all" || mode === "channels";

  return (
    <div class="fade-in">
      {showLocksFamily && sems.length > 0 && (
        <div class="card" style="margin-bottom: 16px">
          <div class="card-head">
            Semaphores
            <span class="muted" style="margin-left: auto">oldest_wait &gt;1s warn, &gt;10s danger</span>
          </div>
          {semsHot.length > 0 && (
            <table>
              <thead>
                <tr>
                  <th>Severity</th>
                  <th>Process</th>
                  <th>Name</th>
                  <th>Permits</th>
                  <th>Waiters</th>
                  <th>Oldest Wait</th>
                  <th>Acquires</th>
                  <th>Avg Wait</th>
                  <th>Max Wait</th>
                  <th>Age</th>
                  <th>Reason</th>
                  <th>Creator</th>
                </tr>
              </thead>
              <tbody>
                {semsHot.map((s, i) => (
                  <tr key={i} class={rowClass(s.severity)}>
                    <td>{severityBadge(s.severity)}</td>
                    <td class="mono">{processRef(s.process, selectedPath)}</td>
                    <td class="mono">
                      <ResourceLink
                        href={resourceHref({ kind: "semaphore", process: s.process, name: s.sem.name })}
                        active={isActivePath(selectedPath, resourceHref({ kind: "semaphore", process: s.process, name: s.sem.name }))}
                        kind="semaphore"
                      >
                        {s.sem.name}
                      </ResourceLink>
                    </td>
                    <td class="num">
                      <span class={classNames(s.sem.permits_available === 0 && "text-amber")}>
                        {s.sem.permits_available}
                      </span>
                      {" / "}{s.sem.permits_total}
                    </td>
                    <td class={classNames("num", s.sem.waiters > 0 && "text-amber")}>{s.sem.waiters}</td>
                    <td class={classNames("num", s.sem.oldest_wait_secs > 1 && "text-amber")}>
                      {s.sem.oldest_wait_secs > 0 ? fmtDuration(s.sem.oldest_wait_secs) : "\u2014"}
                    </td>
                    <td class="num">{s.sem.acquires}</td>
                    <td class="num">{s.sem.avg_wait_secs > 0 ? fmtDuration(s.sem.avg_wait_secs) : "\u2014"}</td>
                    <td class="num">{s.sem.max_wait_secs > 0 ? fmtDuration(s.sem.max_wait_secs) : "\u2014"}</td>
                    <td class="num">{fmtAge(s.sem.age_secs)}</td>
                    <td>{semaphoreReason(s.sem)}</td>
                    <td>{taskRef(s.process, s.sem.creator_task_id, s.sem.creator_task_name, selectedPath)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {semsIdle.length > 0 && (
            <details style="padding: 8px 12px 12px">
              <summary class="muted" style="cursor: pointer">Idle semaphores ({semsIdle.length})</summary>
              <table style="margin-top: 8px">
                <thead>
                  <tr>
                    <th>Severity</th>
                    <th>Process</th>
                    <th>Name</th>
                    <th>Permits</th>
                    <th>Waiters</th>
                    <th>Acquires</th>
                    <th>Age</th>
                  </tr>
                </thead>
                <tbody>
                  {semsIdle.map((s, i) => (
                    <tr key={i}>
                      <td>{severityBadge(s.severity)}</td>
                      <td class="mono">{processRef(s.process, selectedPath)}</td>
                      <td class="mono">
                        <ResourceLink
                          href={resourceHref({ kind: "semaphore", process: s.process, name: s.sem.name })}
                          active={isActivePath(selectedPath, resourceHref({ kind: "semaphore", process: s.process, name: s.sem.name }))}
                          kind="semaphore"
                        >
                          {s.sem.name}
                        </ResourceLink>
                      </td>
                      <td class="num">{s.sem.permits_available} / {s.sem.permits_total}</td>
                      <td class="num">{s.sem.waiters}</td>
                      <td class="num">{s.sem.acquires}</td>
                      <td class="num">{fmtAge(s.sem.age_secs)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </details>
          )}
        </div>
      )}

      {showChannelsFamily && roamChs.length > 0 && (
        <div class="card" style="margin-bottom: 16px">
          <div class="card-head">
            Roam Channels
            <span class="muted" style="margin-left: auto">stale channels (&gt;10s idle) flagged as warn</span>
          </div>
          {roamChsHot.length > 0 && (
            <table>
              <thead>
                <tr>
                  <th>Severity</th>
                  <th>Process</th>
                  <th>Name</th>
                  <th>Direction</th>
                  <th>Queue Depth</th>
                  <th>Age</th>
                  <th>Closed</th>
                  <th>Task</th>
                  <th>Reason</th>
                </tr>
              </thead>
              <tbody>
                {roamChsHot.map((r, i) => (
                  <tr key={i} class={rowClass(r.severity)}>
                    <td>{severityBadge(r.severity)}</td>
                    <td class="mono">{processRef(r.process, selectedPath)}</td>
                    <td class="mono">
                      <ResourceLink
                        href={resourceHref({ kind: "roam_channel", process: r.process, channelId: r.ch.channel_id })}
                        active={isActivePath(selectedPath, resourceHref({ kind: "roam_channel", process: r.process, channelId: r.ch.channel_id }))}
                        kind="roam_channel"
                      >
                        {r.ch.name}
                      </ResourceLink>
                    </td>
                    <td class="mono">{r.ch.direction}</td>
                    <td class="num">{r.ch.queue_depth ?? "\u2014"}</td>
                    <td class="num">{fmtAge(r.ch.age_secs)}</td>
                    <td>{r.ch.closed ? <span class="state-badge state-dropped">closed</span> : "\u2014"}</td>
                    <td>{taskRef(r.process, r.ch.task_id, r.ch.task_name, selectedPath)}</td>
                    <td>{roamChannelReason(r.ch)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {roamChsIdle.length > 0 && (
            <details style="padding: 8px 12px 12px">
              <summary class="muted" style="cursor: pointer">Idle channels ({roamChsIdle.length})</summary>
              <table style="margin-top: 8px">
                <thead>
                  <tr>
                    <th>Severity</th>
                    <th>Process</th>
                    <th>Name</th>
                    <th>Direction</th>
                    <th>Queue Depth</th>
                    <th>Age</th>
                    <th>Task</th>
                  </tr>
                </thead>
                <tbody>
                  {roamChsIdle.map((r, i) => (
                    <tr key={i}>
                      <td>{severityBadge(r.severity)}</td>
                      <td class="mono">{processRef(r.process, selectedPath)}</td>
                      <td class="mono">
                        <ResourceLink
                          href={resourceHref({ kind: "roam_channel", process: r.process, channelId: r.ch.channel_id })}
                          active={isActivePath(selectedPath, resourceHref({ kind: "roam_channel", process: r.process, channelId: r.ch.channel_id }))}
                          kind="roam_channel"
                        >
                          {r.ch.name}
                        </ResourceLink>
                      </td>
                      <td class="mono">{r.ch.direction}</td>
                      <td class="num">{r.ch.queue_depth ?? "\u2014"}</td>
                      <td class="num">{fmtAge(r.ch.age_secs)}</td>
                      <td>{taskRef(r.process, r.ch.task_id, r.ch.task_name, selectedPath)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </details>
          )}
        </div>
      )}

      {showChannelsFamily && mpsc.length > 0 && (
        <div class="card" style="margin-bottom: 16px">
          <div class="card-head">
            MPSC Channels
            <span class="muted" style="margin-left: auto">state-space lens: active, closed, idle</span>
          </div>
          <div class="sync-buckets">
            <div class="sync-bucket"><span class="label">Active</span><span class="value">{mpscSummary.active}</span></div>
            <div class="sync-bucket"><span class="label">Closed (no impact)</span><span class="value">{mpscSummary.closedBenign}</span></div>
            <div class="sync-bucket sync-bucket-danger"><span class="label">Closed (with waiters)</span><span class="value">{mpscSummary.closedWithWaiters}</span></div>
            <div class="sync-bucket"><span class="label">Idle</span><span class="value">{mpscSummary.idle}</span></div>
          </div>

          <div class="sync-groups">
            {mpscGroups.map((group) => (
              <details key={group.name} class="sync-group" open={group.topSeverity !== "idle"}>
                <summary class="sync-group-summary">
                  <div class="sync-group-title">
                    <ResourceLink
                      href={resourceHref({ kind: "mpsc", process: group.items[0]?.process ?? "", name: group.name })}
                      active={group.items.some((m) => isActivePath(selectedPath, resourceHref({ kind: "mpsc", process: m.process, name: m.ch.name })))}
                      kind="mpsc"
                    >
                      {group.name}
                    </ResourceLink>
                    <span class="muted">{group.items.length} instance{group.items.length > 1 ? "s" : ""} across {group.processCount} process{group.processCount > 1 ? "es" : ""}</span>
                  </div>
                  <div class="sync-group-meta">{severityBadge(group.topSeverity)}</div>
                </summary>

                <div class="sync-instance-list">
                  {group.items.map((m, i) => {
                    const backlog = mpscBacklog(m.ch);
                    const anomalous = m.ch.send_waiters > 0 || backlog > 0 || (m.ch.receiver_closed && m.ch.sender_count > 0);
                    return (
                      <article key={`${group.name}:${m.process}:${i}`} class={classNames("sync-instance-card", rowClass(m.severity))}>
                        <div class="sync-instance-head">
                          <div class="sync-instance-title">
                            <ResourceLink
                              href={resourceHref({ kind: "mpsc", process: m.process, name: m.ch.name })}
                              active={isActivePath(selectedPath, resourceHref({ kind: "mpsc", process: m.process, name: m.ch.name }))}
                              kind="mpsc"
                            >
                              {m.ch.name}
                            </ResourceLink>
                            <span class={classNames("state-badge", m.severity === "danger" ? "state-dropped" : m.severity === "warn" ? "state-pending" : "state-empty")}>
                              {mpscStateLabel(m.ch)}
                            </span>
                          </div>
                          <div class="sync-instance-process">{processRef(m.process, selectedPath)} · {fmtAge(m.ch.age_secs)}</div>
                        </div>

                        <div class="sync-instance-body">
                          <div class="sync-kv"><span class="k">Senders</span><span class="v num">{m.ch.sender_count}</span></div>
                          <div class="sync-kv"><span class="k">Waiters</span><span class={classNames("v num", m.ch.send_waiters > 0 && "text-amber")}>{m.ch.send_waiters}</span></div>
                          {anomalous && (
                            <>
                              <div class="sync-kv"><span class="k">Traffic</span><span class="v num">{m.ch.sent} / {m.ch.received}</span></div>
                              {backlog > 0 && <div class="sync-kv"><span class="k">Backlog</span><span class="v num text-amber">{backlog}</span></div>}
                            </>
                          )}
                        </div>

                        <div class="sync-instance-foot">
                          <span>{mpscReason(m.ch)}</span>
                          {m.ch.creator_task_id != null && (
                            <details class="sync-details">
                              <summary>Details</summary>
                              <div>{taskRef(m.process, m.ch.creator_task_id, m.ch.creator_task_name, selectedPath)}</div>
                            </details>
                          )}
                        </div>
                      </article>
                    );
                  })}
                </div>
              </details>
            ))}
          </div>

          {mpscGroups.length === 0 && (
            <div class="empty-state" style="padding: 16px">
              <p>No channels matched the current filter</p>
            </div>
          )}
        </div>
      )}

      {showChannelsFamily && oneshot.length > 0 && (
        <div class="card" style="margin-bottom: 16px">
          <div class="card-head">Oneshot Channels</div>
          {oneshotHot.length > 0 && (
            <table>
              <thead>
                <tr>
                  <th>Severity</th>
                  <th>Process</th>
                  <th>Name</th>
                  <th>State</th>
                  <th>Age</th>
                  <th>Reason</th>
                  <th>Creator</th>
                </tr>
              </thead>
              <tbody>
                {oneshotHot.map((o, i) => (
                  <tr key={i} class={rowClass(o.severity)}>
                    <td>{severityBadge(o.severity)}</td>
                    <td class="mono">{processRef(o.process, selectedPath)}</td>
                    <td class="mono">
                      <ResourceLink
                        href={resourceHref({ kind: "oneshot", process: o.process, name: o.ch.name })}
                        active={isActivePath(selectedPath, resourceHref({ kind: "oneshot", process: o.process, name: o.ch.name }))}
                        kind="oneshot"
                      >
                        {o.ch.name}
                      </ResourceLink>
                    </td>
                    <td><span class={classNames("state-badge", stateClass(o.ch.state))}>{o.ch.state}</span></td>
                    <td class="num">{fmtAge(o.ch.age_secs)}</td>
                    <td>{oneshotReason(o.ch)}</td>
                    <td>{taskRef(o.process, o.ch.creator_task_id, o.ch.creator_task_name, selectedPath)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {oneshotIdle.length > 0 && (
            <details style="padding: 8px 12px 12px">
              <summary class="muted" style="cursor: pointer">Idle channels ({oneshotIdle.length})</summary>
            </details>
          )}
        </div>
      )}

      {showChannelsFamily && watch.length > 0 && (
        <div class="card" style="margin-bottom: 16px">
          <div class="card-head">Watch Channels</div>
          {watchHot.length > 0 && (
            <table>
              <thead>
                <tr>
                  <th>Severity</th>
                  <th>Process</th>
                  <th>Name</th>
                  <th>Changes</th>
                  <th>Receivers</th>
                  <th>Age</th>
                  <th>Reason</th>
                  <th>Creator</th>
                </tr>
              </thead>
              <tbody>
                {watchHot.map((w, i) => (
                  <tr key={i} class={rowClass(w.severity)}>
                    <td>{severityBadge(w.severity)}</td>
                    <td class="mono">{processRef(w.process, selectedPath)}</td>
                    <td class="mono">
                      <ResourceLink
                        href={resourceHref({ kind: "watch", process: w.process, name: w.ch.name })}
                        active={isActivePath(selectedPath, resourceHref({ kind: "watch", process: w.process, name: w.ch.name }))}
                        kind="watch"
                      >
                        {w.ch.name}
                      </ResourceLink>
                    </td>
                    <td class="num">{w.ch.changes}</td>
                    <td class="num">{w.ch.receiver_count}</td>
                    <td class="num">{fmtAge(w.ch.age_secs)}</td>
                    <td>{watchReason(w.ch)}</td>
                    <td>{taskRef(w.process, w.ch.creator_task_id, w.ch.creator_task_name, selectedPath)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {watchIdle.length > 0 && (
            <details style="padding: 8px 12px 12px">
              <summary class="muted" style="cursor: pointer">Idle channels ({watchIdle.length})</summary>
            </details>
          )}
        </div>
      )}

      {showChannelsFamily && once.length > 0 && (
        <div class="card">
          <div class="card-head">OnceCells</div>
          {onceHot.length > 0 && (
            <table>
              <thead>
                <tr>
                  <th>Severity</th>
                  <th>Process</th>
                  <th>Name</th>
                  <th>State</th>
                  <th>Age</th>
                  <th>Init Duration</th>
                  <th>Reason</th>
                </tr>
              </thead>
              <tbody>
                {onceHot.map((o, i) => (
                  <tr key={i} class={rowClass(o.severity)}>
                    <td>{severityBadge(o.severity)}</td>
                    <td class="mono">{processRef(o.process, selectedPath)}</td>
                    <td class="mono">
                      <ResourceLink
                        href={resourceHref({ kind: "once_cell", process: o.process, name: o.ch.name })}
                        active={isActivePath(selectedPath, resourceHref({ kind: "once_cell", process: o.process, name: o.ch.name }))}
                        kind="once_cell"
                      >
                        {o.ch.name}
                      </ResourceLink>
                    </td>
                    <td><span class={classNames("state-badge", stateClass(o.ch.state))}>{o.ch.state}</span></td>
                    <td class="num">{fmtAge(o.ch.age_secs)}</td>
                    <td class="num">{o.ch.init_duration_secs != null ? (o.ch.init_duration_secs * 1000).toFixed(0) + "ms" : "\u2014"}</td>
                    <td>{onceReason(o.ch)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {onceIdle.length > 0 && (
            <details style="padding: 8px 12px 12px">
              <summary class="muted" style="cursor: pointer">Idle cells ({onceIdle.length})</summary>
            </details>
          )}
        </div>
      )}
    </div>
  );
}
