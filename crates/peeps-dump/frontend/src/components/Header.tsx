import type { ProcessDump } from "../types";

interface HeaderProps {
  dumps: ProcessDump[];
  filter: string;
  onFilter: (v: string) => void;
  onRefresh: () => void;
  error: string | null;
}

export function Header({ dumps, filter, onFilter, onRefresh, error }: HeaderProps) {
  const totalTasks = dumps.reduce((s, d) => s + d.tasks.length, 0);
  const totalThreads = dumps.reduce((s, d) => s + d.threads.length, 0);

  return (
    <div class="header">
      <div class="header-brand">
        <span class="accent">peeps</span> dashboard
      </div>
      <div class="header-sep" />
      <div class="header-stats">
        <span>
          <span class="val">{dumps.length}</span> processes
        </span>
        <span>
          <span class="val">{totalTasks}</span> tasks
        </span>
        <span>
          <span class="val">{totalThreads}</span> threads
        </span>
      </div>
      <div class="header-spacer" />
      {error && <span class="header-error">{error}</span>}
      <input
        class="search-box"
        type="text"
        placeholder="Filter..."
        autocomplete="off"
        spellcheck={false}
        value={filter}
        onInput={(e) => onFilter((e.target as HTMLInputElement).value)}
      />
      <button
        class="expand-trigger"
        style="padding: 5px 12px; font-size: 12px"
        onClick={onRefresh}
      >
        Refresh
      </button>
    </div>
  );
}
