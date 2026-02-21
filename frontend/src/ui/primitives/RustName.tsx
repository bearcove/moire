import React from "react";
import "./RustName.css";

// ── Token types ──────────────────────────────────────────────────────────────

export type TokenKind = "sep" | "angle" | "kw" | "lifetime" | "primitive" | "closure" | "punct" | "ws" | "ident";

export interface RustToken {
  kind: TokenKind;
  text: string;
  isFn: boolean;
}

type RawToken = { kind: TokenKind; text: string };

// ── Tokenizer ────────────────────────────────────────────────────────────────

const KEYWORDS = new Set(["as", "dyn", "impl", "mut", "unsafe", "fn", "const", "for", "where"]);
const PRIMITIVES = new Set([
  "u8", "u16", "u32", "u64", "u128", "usize",
  "i8", "i16", "i32", "i64", "i128", "isize",
  "f32", "f64", "bool", "char", "str",
]);

export function tokenizeRustName(name: string): RustToken[] {
  const raw: RawToken[] = [];
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

export interface SlimParts {
  crate: string | null;
  slim: RustToken[];
  closureCount: number;
  wasStripped: boolean;
}

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

function parseSimpleSlim(body: RawToken[], closureCount: number): SlimParts {
  const segs = splitAtTopLevel(body);

  let crate: string | null = null;
  let remaining = segs;
  if (segs[0]?.length === 1 && segs[0][0].kind === "ident") {
    crate = segs[0][0].text;
    remaining = segs.slice(1);
  }

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

  const typeTokens = body.slice(1, asIdx).filter(t => t.kind !== "ws");
  const typeSegs = splitAtTopLevel(typeTokens);
  const typeLastSeg = typeSegs[typeSegs.length - 1] ?? typeTokens;
  const crate = typeSegs[0]?.find(t => t.kind === "ident")?.text ?? null;

  const methodTokens = body.slice(outerClose + 2);
  const methodSegs = splitAtTopLevel(methodTokens);

  const slim = joinSegs([typeLastSeg, ...(methodSegs.length > 0 ? [methodSegs[methodSegs.length - 1]] : [])]);
  return { crate, slim, closureCount, wasStripped: true };
}

export function parseSlim(allTokens: RustToken[]): SlimParts {
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

// ── Renderer ─────────────────────────────────────────────────────────────────

export function RustTokens({ tokens }: { tokens: RustToken[] }) {
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
