import React, { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { Stack, FileRs } from "@phosphor-icons/react";
import type { SnapshotBacktraceFrame } from "../../api/types.generated";
import type { ResolvedSnapshotBacktrace } from "../../snapshot";
import { assignScopeColorRgbByKey, type ScopeColorPair } from "../graph/scopeColors";
import { Source } from "./Source";
import "./BacktraceRenderer.css";

const SYSTEM_PREFIXES = [
  "std::",
  "core::",
  "alloc::",
  "tokio::",
  "tokio_util::",
  "futures::",
  "futures_core::",
  "futures_util::",
  "moire::",
  "moire_trace_capture::",
  "moire_runtime::",
  "moire_tokio::",
];

function isResolved(frame: SnapshotBacktraceFrame): frame is { resolved: { module_path: string; function_name: string; source_file: string; line?: number } } {
  return "resolved" in frame;
}

function isSystemFrame(frame: SnapshotBacktraceFrame): boolean {
  if (!isResolved(frame)) return false;
  return SYSTEM_PREFIXES.some((prefix) => frame.resolved.function_name.startsWith(prefix));
}

function detectAppCrate(frames: SnapshotBacktraceFrame[]): string | null {
  for (const frame of frames) {
    if (!isResolved(frame)) continue;
    if (/(?:^|::)main$/.test(frame.resolved.function_name)) {
      return frame.resolved.function_name.split("::")[0] ?? null;
    }
  }
  return null;
}

// Index is intentional: a backtrace can contain the same frame multiple times (recursion).
function frameKey(frame: SnapshotBacktraceFrame, index: number): string {
  if (isResolved(frame)) {
    return `r:${index}:${frame.resolved.module_path}:${frame.resolved.function_name}:${frame.resolved.source_file}:${frame.resolved.line ?? ""}`;
  }
  return `u:${index}:${frame.unresolved.module_path}:${frame.unresolved.rel_pc}`;
}

/** Fast crate extraction without tokenizing — handles trait impls too. */
function extractCrate(functionName: string): string | null {
  if (functionName.startsWith("<")) {
    const m = functionName.match(/^<([a-zA-Z_][a-zA-Z0-9_]*)::/);
    return m?.[1] ?? null;
  }
  const sep = functionName.indexOf("::");
  return sep === -1 ? null : functionName.slice(0, sep);
}

// ── Rust name tokenizer ──────────────────────────────────────────────────────

type TokenKind = "sep" | "angle" | "kw" | "lifetime" | "primitive" | "closure" | "punct" | "ws" | "ident";

interface RustToken {
  kind: TokenKind;
  text: string;
  isFn: boolean;
}

const KEYWORDS = new Set(["as", "dyn", "impl", "mut", "unsafe", "fn", "const", "for", "where"]);
const PRIMITIVES = new Set([
  "u8", "u16", "u32", "u64", "u128", "usize",
  "i8", "i16", "i32", "i64", "i128", "isize",
  "f32", "f64", "bool", "char", "str",
]);

function tokenizeRustName(name: string): RustToken[] {
  const raw: { kind: TokenKind; text: string }[] = [];
  let i = 0;

  while (i < name.length) {
    if (name.startsWith("::", i)) {
      raw.push({ kind: "sep", text: "::" }); i += 2; continue;
    }
    if (name.startsWith("{{closure}}", i)) {
      raw.push({ kind: "closure", text: "{{closure}}" }); i += 11; continue;
    }
    if (name[i] === "{") {
      const end = name.indexOf("}", i + 1);
      if (end !== -1) {
        raw.push({ kind: "closure", text: name.slice(i, end + 1) }); i = end + 1; continue;
      }
    }
    if (name[i] === "'") {
      let j = i + 1;
      while (j < name.length && /[a-zA-Z0-9_]/.test(name[j])) j++;
      raw.push({ kind: "lifetime", text: name.slice(i, j) }); i = j; continue;
    }
    if (name.startsWith("->", i)) {
      raw.push({ kind: "punct", text: "->" }); i += 2; continue;
    }
    if (name[i] === " " || name[i] === "\t") {
      raw.push({ kind: "ws", text: " " }); i++; continue;
    }
    if (/[a-zA-Z_]/.test(name[i])) {
      let j = i + 1;
      while (j < name.length && /[a-zA-Z0-9_]/.test(name[j])) j++;
      const text = name.slice(i, j);
      const kind: TokenKind = KEYWORDS.has(text) ? "kw" : PRIMITIVES.has(text) ? "primitive" : "ident";
      raw.push({ kind, text }); i = j; continue;
    }
    if (name[i] === "<" || name[i] === ">") {
      raw.push({ kind: "angle", text: name[i] }); i++; continue;
    }
    if ("()[]*&,+=!".includes(name[i])) {
      raw.push({ kind: "punct", text: name[i] }); i++; continue;
    }
    if (/[0-9]/.test(name[i])) {
      let j = i + 1;
      while (j < name.length && /[0-9a-fA-FxX_]/.test(name[j])) j++;
      raw.push({ kind: "ident", text: name.slice(i, j) }); i = j; continue;
    }
    raw.push({ kind: "punct", text: name[i] }); i++;
  }

  // Semantic fn boundary: skip trailing closure chain, find last :: before the real fn name.
  let end = raw.length - 1;
  while (end >= 0 && raw[end].kind === "closure") {
    end--;
    if (end >= 0 && raw[end].kind === "sep") end--;
  }
  let lastSepIdx = -1;
  for (let k = end; k >= 0; k--) {
    if (raw[k].kind === "sep") { lastSepIdx = k; break; }
  }

  return raw.map((tok, idx) => ({ ...tok, isFn: idx > lastSepIdx }));
}

// ── Slim name parser ─────────────────────────────────────────────────────────

type RawToken = { kind: TokenKind; text: string };

/** Split raw tokens at top-level `::` (not inside <> brackets). */
function splitAtTopLevel(toks: RawToken[]): RawToken[][] {
  const segs: RawToken[][] = [];
  let depth = 0, start = 0;
  for (let i = 0; i < toks.length; i++) {
    if (toks[i].kind === "angle" && toks[i].text === "<") depth++;
    else if (toks[i].kind === "angle" && toks[i].text === ">") depth--;
    else if (toks[i].kind === "sep" && depth === 0) {
      segs.push(toks.slice(start, i));
      start = i + 1;
    }
  }
  segs.push(toks.slice(start));
  return segs.filter(s => s.length > 0);
}

/** Join segments into RustTokens, marking only the last segment as isFn. */
function joinSegs(segs: RawToken[][]): RustToken[] {
  const out: RustToken[] = [];
  segs.forEach((seg, si) => {
    const isFnSeg = si === segs.length - 1;
    seg.forEach(t => out.push({ ...t, isFn: isFnSeg }));
    if (si < segs.length - 1) out.push({ kind: "sep", text: "::", isFn: false });
  });
  return out;
}

interface SlimParts {
  crate: string | null;
  slim: RustToken[];
  closureCount: number;
  wasStripped: boolean;
}

function parseSlim(allTokens: RustToken[]): SlimParts {
  const raw = allTokens.map(t => ({ kind: t.kind, text: t.text }));

  // Strip trailing closures
  let closureCount = 0;
  let end = raw.length;
  while (end > 0 && raw[end - 1].kind === "closure") {
    closureCount++;
    end--;
    if (end > 0 && raw[end - 1].kind === "sep") end--;
  }
  const body = raw.slice(0, end);

  if (body[0]?.kind === "angle" && body[0].text === "<") {
    return parseTraitImplSlim(body, closureCount);
  }
  return parseSimpleSlim(body, closureCount);
}

function parseSimpleSlim(body: RawToken[], closureCount: number): SlimParts {
  const segs = splitAtTopLevel(body);

  let crate: string | null = null;
  let remaining = segs;
  if (segs[0]?.length === 1 && segs[0][0].kind === "ident") {
    crate = segs[0][0].text;
    remaining = segs.slice(1);
  }

  // Last 2 segments
  const last2 = remaining.slice(-2);
  const slim = joinSegs(last2);
  const wasStripped = remaining.length > 2 || crate != null;

  return { crate, slim, closureCount, wasStripped };
}

function parseTraitImplSlim(body: RawToken[], closureCount: number): SlimParts {
  // Find `as` at depth 1 and the closing `>`
  let depth = 0, asIdx = -1, outerClose = -1;
  for (let i = 0; i < body.length; i++) {
    if (body[i].kind === "angle" && body[i].text === "<") depth++;
    else if (body[i].kind === "angle" && body[i].text === ">") {
      depth--;
      if (depth === 0) { outerClose = i; break; }
    } else if (depth === 1 && body[i].kind === "kw" && body[i].text === "as") {
      asIdx = i;
    }
  }

  const fallbackCrate = body.find(t => t.kind === "ident")?.text ?? null;
  if (asIdx === -1 || outerClose === -1) {
    return { crate: fallbackCrate, slim: joinSegs([body]), closureCount, wasStripped: false };
  }

  // Type = body[1..asIdx] (inside <, before `as`)
  const typeTokens = body.slice(1, asIdx).filter(t => t.kind !== "ws");
  const typeSegs = splitAtTopLevel(typeTokens);
  const typeLastSeg = typeSegs[typeSegs.length - 1] ?? typeTokens;
  const crate = typeSegs[0]?.find(t => t.kind === "ident")?.text ?? null;

  // Method = tokens after `>::`
  const methodTokens = body.slice(outerClose + 2);
  const methodSegs = splitAtTopLevel(methodTokens);

  const slim = joinSegs([typeLastSeg, ...(methodSegs.length > 0 ? [methodSegs[methodSegs.length - 1]] : [])]);
  return { crate, slim, closureCount, wasStripped: true };
}

// ── Renderer components ──────────────────────────────────────────────────────

function RustTokens({ tokens }: { tokens: RustToken[] }) {
  return (
    <>
      {tokens.map((tok, idx) => (
        // eslint-disable-next-line react/no-array-index-key
        <span key={idx} className={`bt-tok bt-tok--${tok.kind}${tok.isFn ? " bt-tok--fn" : ""}`}>
          {tok.text}
        </span>
      ))}
    </>
  );
}

function zedUrl(path: string, line?: number): string {
  return line != null ? `zed://file${path}:${line}` : `zed://file${path}`;
}

// ── Badge ────────────────────────────────────────────────────────────────────

export function BacktraceBadge({
  backtrace,
  onExpand,
}: {
  backtrace: ResolvedSnapshotBacktrace;
  onExpand: () => void;
}) {
  const topFrame = useMemo(
    () =>
      backtrace.frames.find((f) => isResolved(f) && !isSystemFrame(f))
      ?? backtrace.frames.find((f) => isResolved(f))
      ?? backtrace.frames[0],
    [backtrace.frames],
  );

  const resolvedTop = topFrame && isResolved(topFrame) ? topFrame.resolved : null;
  const location = resolvedTop?.source_file
    ? `${resolvedTop.source_file.split("/").pop() ?? resolvedTop.source_file}${resolvedTop.line != null ? `:${resolvedTop.line}` : ""}`
    : null;
  const href = resolvedTop?.source_file ? zedUrl(resolvedTop.source_file, resolvedTop.line) : null;

  return (
    <span className="bt-badge">
      {location && href ? (
        <a
          className="bt-badge-location"
          href={href}
          title={`Open ${resolvedTop!.source_file}${resolvedTop!.line != null ? `:${resolvedTop!.line}` : ""} in Zed`}
        >
          <FileRs size={12} weight="bold" className="bt-badge-file-icon" />
          {location}
        </a>
      ) : (
        <span className="bt-badge-location bt-badge-location--pending">pending…</span>
      )}
      <button type="button" className="bt-badge-expand" onClick={onExpand} title="View full backtrace">
        <Stack size={11} weight="bold" />
      </button>
    </span>
  );
}

// ── Panel ────────────────────────────────────────────────────────────────────

export function BacktracePanel({
  backtrace,
  onClose,
}: {
  backtrace: ResolvedSnapshotBacktrace;
  onClose: () => void;
}) {
  const [showSystem, setShowSystem] = useState(false);

  const appCrate = useMemo(() => detectAppCrate(backtrace.frames), [backtrace.frames]);

  const crateColors = useMemo(() => {
    const crates = backtrace.frames
      .filter(isResolved)
      .map(f => extractCrate(f.resolved.function_name) ?? "")
      .filter(Boolean);
    return assignScopeColorRgbByKey(crates);
  }, [backtrace.frames]);

  const systemCount = useMemo(
    () => backtrace.frames.filter((f) => isResolved(f) && isSystemFrame(f)).length,
    [backtrace.frames],
  );
  const unresolvedCount = useMemo(
    () => backtrace.frames.filter((f) => !isResolved(f)).length,
    [backtrace.frames],
  );

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div className="bt-overlay" role="dialog" aria-modal="true" onClick={onClose}>
      <div className="bt-dialog" onClick={(event) => event.stopPropagation()}>
        <div className="bt-dialog-header">
          <span className="bt-dialog-title">Backtrace</span>
          <span className="bt-dialog-meta">
            {backtrace.frames.length} frames
            {unresolvedCount > 0 && <> · {unresolvedCount} unresolved</>}
            {appCrate && <> · app: <span className="bt-dialog-app-crate">{appCrate}</span></>}
          </span>
          <button
            type="button"
            className="bt-section-toggle"
            onClick={() => setShowSystem((v) => !v)}
          >
            {showSystem ? "Hide" : "Show"} system ({systemCount})
          </button>
          <button type="button" className="bt-dialog-close" onClick={onClose}>
            Esc
          </button>
        </div>

        <div className="bt-frame-list">
          {backtrace.frames.map((frame, index) => {
            if (!isResolved(frame)) {
              return (
                // eslint-disable-next-line react/no-array-index-key -- index disambiguates recursive frames
                <div className="bt-frame-row bt-frame-row--unresolved" key={frameKey(frame, index)}>
                  <span className="bt-fn bt-fn--unresolved">
                    {frame.unresolved.module_path}+0x{frame.unresolved.rel_pc.toString(16)}
                  </span>
                  <span className="bt-reason">—</span>
                </div>
              );
            }
            if (isSystemFrame(frame)) {
              if (!showSystem) return null;
              // eslint-disable-next-line react/no-array-index-key -- index disambiguates recursive frames
              return <FrameRow key={frameKey(frame, index)} frame={frame} crateColors={crateColors} appCrate={null} isSystem />;
            }
            // eslint-disable-next-line react/no-array-index-key -- index disambiguates recursive frames
            return <FrameRow key={frameKey(frame, index)} frame={frame} crateColors={crateColors} appCrate={appCrate} />;
          })}
        </div>
      </div>
    </div>
  );
}

// ── Frame row ────────────────────────────────────────────────────────────────

function FrameRow({
  frame,
  crateColors,
  appCrate,
  isSystem = false,
}: {
  frame: SnapshotBacktraceFrame;
  crateColors: Map<string, ScopeColorPair>;
  appCrate: string | null;
  isSystem?: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  if (!isResolved(frame)) return null;

  const { function_name, source_file, line } = frame.resolved;

  // eslint-disable-next-line react-hooks/rules-of-hooks
  const allTokens = useMemo(() => tokenizeRustName(function_name), [function_name]);
  // eslint-disable-next-line react-hooks/rules-of-hooks
  const { crate, slim, closureCount, wasStripped } = useMemo(() => parseSlim(allTokens), [allTokens]);

  const crateColor = crate ? crateColors.get(crate) : null;
  const isApp = appCrate != null && crate === appCrate;
  const sourceStr = source_file.length > 0
    ? (line != null ? `${source_file}:${line}` : source_file)
    : null;

  const rowClass = [
    "bt-frame-row",
    isSystem && "bt-frame-row--system",
    isApp && "bt-frame-row--app",
  ].filter(Boolean).join(" ");

  const pillStyle = crateColor
    ? { "--scope-rgb-light": crateColor.light, "--scope-rgb-dark": crateColor.dark } as React.CSSProperties
    : {};

  return (
    <div className={rowClass}>
      <div className="bt-fn-line">
        {crate && (
          <span className="bt-crate-pill" style={pillStyle}>
            {crate}
          </span>
        )}
        <span
          className={`bt-fn${wasStripped ? " bt-fn--expandable" : ""}`}
          title={function_name}
          onClick={wasStripped ? () => setExpanded(v => !v) : undefined}
          role={wasStripped ? "button" : undefined}
        >
          <RustTokens tokens={expanded ? allTokens : slim} />
        </span>
        {closureCount > 0 && (
          <span className="bt-closure-pill">{"{}"}</span>
        )}
      </div>
      {sourceStr && <Source source={sourceStr} />}
    </div>
  );
}

// ── Widget ───────────────────────────────────────────────────────────────────

export function BacktraceRenderer({
  backtrace,
}: {
  backtrace: ResolvedSnapshotBacktrace;
}) {
  const [open, setOpen] = useState(false);

  return (
    <>
      <BacktraceBadge backtrace={backtrace} onExpand={() => setOpen(true)} />
      {open && createPortal(
        <BacktracePanel backtrace={backtrace} onClose={() => setOpen(false)} />,
        document.body,
      )}
    </>
  );
}
