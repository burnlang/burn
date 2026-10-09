"use strict";
const $fs = require("fs");
const $cp = require("child_process");
const K = { ERR: 0, VOID: 1, NULL: 2, INT: 3, FLOAT: 4, BOOL: 5, STR: 6, ANY: 7, ARR_ANY: 8, MAP_STR_ANY: 9, ARR_STR: 10, ARR_INT: 11 };
class BurnError extends Error {}
class BurnExit extends Error { constructor(c) { super("exit"); this.code = c; } }
class $Box { constructor(b, v) { this.b = b; this.v = v; } }
class $Map { constructor(t) { this.t = t; this.m = new Map(); } }
let $out = [];
let $outLen = 0;
function $flush() { if ($out.length) { $fs.writeSync(1, $out.join("")); $out = []; $outLen = 0; } }
function $write(s) { $out.push(s); $outLen += s.length; if ($outLen > 65536) $flush(); }
function $errHelp(msg) {
  let m;
  if (msg.startsWith("index ") || msg.startsWith("string index ")) {
    m = /\(length (\d+)/.exec(msg);
    if (!m) return null;
    const n = Number(m[1]);
    return n === 0 ? "it is empty, so there is nothing to read; check `len(...) > 0` first" : "valid indexes go from 0 to " + (n - 1) + "; check the index against `len(...)` first";
  }
  const $range = name => { if (name === "int") return "int holds values from -9223372036854775808 to 9223372036854775807"; const n = $NUMS.find(x => x && x[0] === name && x.length > 1); return n ? name + " holds values from " + n[1] + " to " + (n[3] || n[2]) : null; };
  if (msg.startsWith("integer overflow: ")) { const r = $range(msg.split(" ").pop()); return r === null ? null : r + "; use a wider type, or `wrappingAdd`, `wrappingSub` and `wrappingMul` to wrap around"; }
  if (msg.startsWith("cannot convert ") && !msg.startsWith("cannot convert \"")) { const r = $range(msg.split(" ").pop()); return r === null ? null : r + "; check the value before converting it with `as`"; }
  if (msg.startsWith("cannot shift by ")) return "the shift amount must be from 0 to 63";
  if (msg === "integer overflow") return "the result does not fit in `int` (-9223372036854775808 to 9223372036854775807); use `float` for larger numbers";
  if (msg === "division by zero") return "check that the divisor is not 0 before dividing";
  if (msg === "unexpected null value") return "the value was null; check it with `if (x != null)` instead of using `!!`";
  if (msg === "cannot index null") return "the value was null; check it with `if (x != null)` before indexing";
  if (msg.startsWith("key ") && msg.endsWith(" not found in map")) return "check the key with `has(map, key)` first, or pass a default: `get(map, key, fallback)`";
  if (msg === "pop from empty array") return "check `len(items) > 0` before calling `pop`";
  if (msg.startsWith("cannot convert ")) return "the text is not a number; check it first or handle the bad input";
  if (msg.startsWith("cannot cast value")) return "check the type with `is` before casting with `as`";
  if (msg === "this object was destroyed and can no longer be used") return "another variable or function destroyed this object; do not use it after `destroy`";
  if (msg === "this object was already destroyed") return "each object can be destroyed only once";
  if (msg === "function ended without returning a value") return "make sure every path through the function ends with `return`";
  if (msg.startsWith("stack overflow")) return "a function probably calls itself without ever stopping; check its base case";
  return null;
}
function $srcLine(l) {
  const m = /^(.*):(\d+):(\d+)$/.exec(l);
  if (!m || typeof require !== "function") return null;
  try {
    const text = require("fs").readFileSync(m[1], "utf8").split("\n")[Number(m[2]) - 1];
    return text === undefined ? null : [Number(m[2]), Number(m[3]), text.replace(/\r$/, "")];
  } catch (e) { return null; }
}
function $errText(msg, loc) {
  let out = "runtime error: " + msg;
  const h = $errHelp(msg), l = $LOCS[loc];
  if (l === undefined) return h === null ? out : out + "\n  = help: " + h;
  const s = $srcLine(l);
  if (s === null) return out + "\n --> " + l + (h === null ? "" : "\n  = help: " + h);
  const g = " ".repeat(String(s[0]).length);
  const pad = Array.from(s[2]).slice(0, s[1] - 1).map(c => c === "\t" ? "\t" : " ").join("");
  out += "\n" + g + "--> " + l + "\n" + g + " |\n" + s[0] + " | " + s[2] + "\n" + g + " | " + pad + "^";
  return h === null ? out : out + "\n" + g + " = help: " + h;
}
function $err(msg, loc) { $flush(); throw new BurnError($errText(msg, loc)); }
function $d(t) { return $T[t] || ["err"]; }
function $unboxed(t) { const k = $d(t)[0]; return k === "int" || k === "float" || k === "num" || k === "bool" || k === "enum" || k === "fun" || k === "void"; }
function $f32str(f) {
  if (!Number.isFinite(f) || (Number.isInteger(f) && Math.abs(f) < 1e16)) return $fstr(f);
  let s = String(f);
  for (let p = 1; p <= 9; p++) { const c = f.toPrecision(p); if (Math.fround(parseFloat(c)) === f) { s = String(parseFloat(c)); break; } }
  if (Math.abs(f) >= 1e16 || (f !== 0 && Math.abs(f) < 1e-6)) return parseFloat(s).toExponential().replace("e+", "e");
  return s;
}
function $numstr(v, name) { return name === "float32" ? $f32str(v) : String(v); }
const $NUMS = [null, ["int8", -128, 127], ["uint8", 0, 255], ["int16", -32768, 32767], ["uint16", 0, 65535], ["int32", -2147483648, 2147483647], ["uint32", 0, 4294967295], ["uint64", 0, 18446744073709551615, "18446744073709551615"], ["float32"]];
function $wrap(op, a, b, u) { const x = BigInt(a), y = BigInt(b); const r = op === "+" ? x + y : op === "-" ? x - y : x * y; return Number(u ? BigInt.asUintN(64, r) : BigInt.asIntN(64, r)); }
function $u(x) { return x < 0 ? Number(BigInt.asUintN(64, BigInt(x))) : x; }
function $nwrap(v, c) {
  if (c === 8) return Math.fround(v);
  if (c === 7) return $u(v);
  const bits = c <= 2 ? 8 : c <= 4 ? 16 : 32;
  return Number(c % 2 === 1 ? BigInt.asIntN(bits, BigInt(v)) : BigInt.asUintN(bits, BigInt(v)));
}
function $tidOf(v) { if (v instanceof $Box) return v.b; if (v instanceof $Map) return v.t; if (Array.isArray(v) && v.$rec) return v[0]; return K.ERR; }
function $rec(tid, fields) { const o = [tid, ...fields]; o.$rec = true; return o; }
function $implements(c, i) { const d = $d(c); return d[0] === "rec" && d[4].indexOf(i) >= 0; }
function $fstr(f) {
  if (Number.isNaN(f)) return "NaN";
  if (f === Infinity) return "Infinity";
  if (f === -Infinity) return "-Infinity";
  if (Number.isInteger(f) && Math.abs(f) < 1e16) return f.toFixed(1);
  if (Math.abs(f) >= 1e16 || (f !== 0 && Math.abs(f) < 1e-6)) return f.toExponential().replace("e+", "e");
  return String(f);
}
function $quote(s) { return JSON.stringify(s); }
function $fmt(v, t, nested, depth) {
  if (depth > 64) return "...";
  const d = $d(t);
  switch (d[0]) {
    case "int": return String(Math.trunc(v));
    case "num": return $numstr(v, d[1]);
    case "float": return $fstr(v);
    case "bool": return v ? "true" : "false";
    case "str": return nested ? $quote(v) : v;
    case "void": case "null": return "null";
    case "fun": return "<fun>";
    case "future": return "<future>";
    case "any": return v === null ? "null" : $fmt(v.v, v.b, nested, depth + 1);
    case "opt": return v === null ? "null" : ($unboxed(d[1]) ? $fmt(v.v, d[1], nested, depth + 1) : $fmt(v, d[1], nested, depth + 1));
    case "enum": return d[2][v] !== undefined ? d[2][v] : String(v);
    case "arr": return "[" + v.map(x => $fmt(x, d[1], true, depth + 1)).join(", ") + "]";
    case "map": { const parts = []; for (const [k, val] of v.m.values()) parts.push($fmt(k, d[1], true, depth + 1) + ": " + $fmt(val, d[2], true, depth + 1)); return "{" + parts.join(", ") + "}"; }
    case "iface": return v === null ? "null" : $fmt(v, v[0], nested, depth);
    case "rec": {
      if (v.$dead) return "<destroyed " + $tname(v[0]) + ">";
      if (v[0] !== t && $d(v[0])[0] === "rec") return $fmt(v, v[0], nested, depth);
      if (d[1].includes(".")) return d[2].length ? d[1] + "(" + d[2].map((f, i) => $fmt(v[i + 1], f[1], true, depth + 1)).join(", ") + ")" : d[1];
      const fs = d[2].map((f, i) => f[0] + ": " + $fmt(v[i + 1], f[1], true, depth + 1));
      const body = fs.length ? "{ " + fs.join(", ") + " }" : "{}";
      return d[1] ? d[1] + " " + body : body;
    }
    default: return "<error>";
  }
}
function $eq(a, b, t) {
  const d = $d(t);
  switch (d[0]) {
    case "int": case "float": case "num": case "bool": case "enum": case "str": case "void": case "null": return a === b;
    case "fun": return a === b || (a !== null && b !== null && a.length === b.length && a.every((x, i) => x === b[i]));
    case "any":
      if (a === null || b === null) return a === b;
      if (a.b !== b.b) { const n = x => { const k = $d(x.b)[0]; return k === "int" || k === "float" || k === "num"; }; return n(a) && n(b) && a.v === b.v; }
      return $eq(a.v, b.v, a.b);
    case "opt": if (a === null || b === null) return a === b; return $unboxed(d[1]) ? $eq(a.v, b.v, d[1]) : $eq(a, b, d[1]);
    case "arr": return a.length === b.length && a.every((x, i) => $eq(x, b[i], d[1]));
    case "map": { if (a.m.size !== b.m.size) return false; for (const [k, [, va]] of a.m) { const e = b.m.get(k); if (!e || !$eq(va, e[1], d[2])) return false; } return true; }
    case "rec": if (a === b) return true; if (a === null || b === null || a[0] !== b[0]) return false; return d[2].every((f, i) => $eq(a[i + 1], b[i + 1], f[1]));
    case "iface": if (a === null || b === null || a[0] !== b[0]) return a === b; return $eq(a, b, a[0]);
    default: return a === b;
  }
}
function $cmp(a, b, t) {
  const k = $d(t)[0];
  if (k === "any") { if (a === null || b === null) return 0; return a.b === b.b ? $cmp(a.v, b.v, a.b) : (a.v < b.v ? -1 : a.v > b.v ? 1 : 0); }
  if (k === "bool") return (a ? 1 : 0) - (b ? 1 : 0);
  return a < b ? -1 : a > b ? 1 : 0;
}
function $chars(s) { return /^[\x00-\x7f]*$/.test(s) ? s.split("") : Array.from(s); }
function $mk(m, k) { const kd = $d($d(m.t)[1])[0]; if (kd === "any") return k === null ? "n" : $d(k.b)[0] + ":" + String(k.v); return k; }
function $box(v, t) {
  const d = $d(t);
  switch (d[0]) {
    case "any": return v;
    case "null": case "void": return null;
    case "opt": if (v === null) return null; return $unboxed(d[1]) ? v : $box(v, d[1]);
    case "iface": return v === null ? null : new $Box(v[0], v);
    case "rec": return new $Box(v[0], v);
    default: return new $Box(t, v);
  }
}
function $dynMatch(actual, to) {
  if (actual === to) return true;
  const d = $d(to);
  if (d[0] === "iface" || d[0] === "rec") return $implements(actual, to);
  if (d[0] === "any") return true;
  if (d[0] === "opt") return $dynMatch(actual, d[1]);
  return false;
}
function $isType(v, from, to) {
  if ($d(to)[0] === "null") return v === null;
  const d = $d(from);
  switch (d[0]) {
    case "any": return v !== null && $dynMatch(v.b, to);
    case "opt": if (v === null) return false; return $unboxed(d[1]) ? $dynMatch(d[1], to) : $isType(v, d[1], to);
    case "iface": case "rec": return v !== null && $dynMatch(v[0], to);
    default: return $dynMatch(from, to);
  }
}
function $tname(t) {
  const d = $d(t);
  switch (d[0]) {
    case "int": return "int"; case "num": return d[1]; case "float": return "float"; case "bool": return "bool"; case "str": return "string";
    case "any": return "any"; case "void": return "void"; case "null": return "null"; case "fun": return "fun";
    case "arr": return "[" + $tname(d[1]) + "]";
    case "map": return "{" + $tname(d[1]) + ": " + $tname(d[2]) + "}";
    case "opt": return $tname(d[1]) + "?";
    case "future": return "Future<" + $tname(d[1]) + ">";
    case "rec": return d[1] || "{" + d[2].map(f => f[0] + ": " + $tname(f[1])).join(", ") + "}";
    case "iface": case "enum": return d[1];
    default: return "<error>";
  }
}
function $payload(v, from, to) {
  const d = $d(from);
  if (d[0] === "any") { const td = $d(to); if (td[0] === "any") return v; if (td[0] === "opt") return $unboxed(td[1]) ? v : v.v; return v.v; }
  if (d[0] === "opt") return $unboxed(d[1]) ? v.v : v;
  return v;
}
function $f2i(x) { if (Number.isNaN(x) || !Number.isFinite(x)) return -9223372036854775808; return Math.trunc(x); }
function $ov(r, l) { if (r > 9223372036854775807 || r < -9223372036854775808) $err("integer overflow", l); return r; }
function $bit(op, a, b, l) {
  const x = BigInt.asIntN(64, BigInt(a));
  let r;
  switch (op) {
    case "&": r = x & BigInt(b); break;
    case "|": r = x | BigInt(b); break;
    case "^": r = x ^ BigInt(b); break;
    case "~": r = ~x; break;
    default:
      if (b < 0 || b > 63) $err("cannot shift by " + b, l);
      if (op === "<<") r = x << BigInt(b);
      else if (op === ">>") r = x >> BigInt(b);
      else r = BigInt.asUintN(64, x) >> BigInt(b);
  }
  return Number(BigInt.asIntN(64, r));
}
function $idiv(a, b, l) { if (b === 0) $err("division by zero", l); return $ov(Math.trunc(a / b), l); }
function $imod(a, b, l) { if (b === 0) $err("division by zero", l); return a % b; }
function $idx(a, i, l) { if (i < 0 || i >= a.length || !Number.isInteger(i)) $err("index " + i + " out of bounds (length " + a.length + ")", l); return a[i]; }
function $sidx(a, i, v, l) { if (i < 0 || i >= a.length || !Number.isInteger(i)) $err("index " + i + " out of bounds (length " + a.length + ")", l); a[i] = v; return v; }
function $httpSplit(raw) {
  let rest = raw;
  for (;;) {
    let p = rest.indexOf("\r\n\r\n");
    const head = p >= 0 ? rest.slice(0, p) : rest;
    const body = p >= 0 ? rest.slice(p + 4) : "";
    const lines = head.split(/\r?\n/);
    const status = parseInt((lines[0] || "").split(/\s+/)[1] || "0", 10) || 0;
    const tunnel = (lines[0] || "").toLowerCase().includes("connection established");
    if ((Math.floor(status / 100) === 1 || tunnel) && body.startsWith("HTTP/")) { rest = body; continue; }
    return [String(status), body, lines.slice(1).join("\n")];
  }
}
let $seed = Date.now() >>> 0 || 1;
function $rand() { $seed ^= $seed << 13; $seed >>>= 0; $seed ^= $seed >>> 17; $seed ^= $seed << 5; $seed >>>= 0; return $seed / 4294967296; }
const $start = process.hrtime.bigint();
function $jsonToAny(v) {
  if (v === null) return null;
  if (Array.isArray(v)) return new $Box(K.ARR_ANY, v.map($jsonToAny));
  switch (typeof v) {
    case "number": return new $Box(Number.isInteger(v) ? K.INT : K.FLOAT, v);
    case "string": return new $Box(K.STR, v);
    case "boolean": return new $Box(K.BOOL, v);
    default: { const m = new $Map(K.MAP_STR_ANY); for (const k of Object.keys(v)) m.m.set(k, [k, $jsonToAny(v[k])]); return new $Box(K.MAP_STR_ANY, m); }
  }
}
function $toJson(v, t) {
  const d = $d(t);
  switch (d[0]) {
    case "int": return String(Math.trunc(v));
    case "num": return Number.isFinite(v) ? $numstr(v, d[1]) : "null";
    case "float": return Number.isFinite(v) ? $fstr(v) : "null";
    case "bool": return v ? "true" : "false";
    case "str": return JSON.stringify(v);
    case "enum": return JSON.stringify(d[2][v] || "");
    case "any": return v === null ? "null" : $toJson(v.v, v.b);
    case "opt": return v === null ? "null" : ($unboxed(d[1]) ? $toJson(v.v, d[1]) : $toJson(v, d[1]));
    case "arr": return "[" + v.map(x => $toJson(x, d[1])).join(",") + "]";
    case "map": { const p = []; for (const [k, val] of v.m.values()) p.push(JSON.stringify($fmt(k, d[1], false, 0)) + ":" + $toJson(val, d[2])); return "{" + p.join(",") + "}"; }
    case "rec": { if (v.$dead) return "null"; if (v[0] !== t && $d(v[0])[0] === "rec") return $toJson(v, v[0]); return "{" + d[2].map((f, i) => JSON.stringify(f[0]) + ":" + $toJson(v[i + 1], f[1])).join(",") + "}"; }
    case "iface": return v === null ? "null" : $toJson(v, v[0]);
    default: return "null";
  }
}
function $localTime() {
  const n = new Date();
  return [n.getFullYear(), n.getMonth() + 1, n.getDate(), n.getHours(), n.getMinutes(), n.getSeconds(), n.getMilliseconds(), n.getDay(), -n.getTimezoneOffset() * 60];
}
function $readLine(prompt) {
  $write(prompt); $flush();
  const buf = Buffer.alloc(1); const bytes = [];
  for (;;) { let n = 0; try { n = $fs.readSync(0, buf, 0, 1, null); } catch (e) { break; } if (n === 0 || buf[0] === 10) break; bytes.push(buf[0]); }
  return Buffer.from(bytes).toString("utf8").replace(/\r+$/, "");
}
const $args = process.argv.slice(2);
const $R = {
  StrConcat: (a, b) => a + b,
  StrAppend: (a, b) => a + b,
  StrEq: (a, b) => a === b,
  StrCmp: (a, b) => (a < b ? -1 : a > b ? 1 : 0),
  StrLen: s => $chars(s).length,
  StrIndex: (s, i, l) => { const c = $chars(s); if (i < 0 || i >= c.length) $err("string index " + i + " out of bounds (length " + c.length + ")", l); return c[i]; },
  StrSub: (s, a, b) => { const c = $chars(s); const n = c.length; const e = Math.max(0, Math.min(b, n)); const st = Math.max(0, Math.min(a, e)); return c.slice(st, e).join(""); },
  StrFind: (s, p) => { const i = s.indexOf(p); return i < 0 ? -1 : Array.from(s.slice(0, i)).length; },
  StrContains: (s, p) => s.includes(p),
  StrReplace: (s, a, b) => (a === "" ? s : s.split(a).join(b)),
  StrSplit: (s, p) => (p === "" ? $chars(s) : s.split(p)),
  StrTrim: s => s.trim(),
  StrUpper: s => s.toUpperCase(),
  StrLower: s => s.toLowerCase(),
  StrStarts: (s, p) => s.startsWith(p),
  StrEnds: (s, p) => s.endsWith(p),
  StrRepeat: (s, n) => s.repeat(Math.max(0, n)),
  StrChars: s => $chars(s),
  StrToBytes: (s, t) => Array.from(new TextEncoder().encode(s)),
  StrFromBytes: a => new TextDecoder().decode(Uint8Array.from(a)),
  CharClass: (s, k) => s.length > 0 && [/^\p{Alphabetic}+$/u, /^[0-9]+$/, /^[\p{Alphabetic}\p{N}]+$/u, /^\s+$/u][k].test(s),
  StrCode: s => (s.length ? s.codePointAt(0) : -1),
  StrFromCode: n => { try { return String.fromCodePoint(n); } catch (e) { return ""; } },
  ToStr: (v, t) => $fmt(v, t, false, 0),
  ParseInt: (s, l) => { const t = s.trim(); if (/^[+-]?\d+$/.test(t)) return parseInt(t, 10); const f = Number(t); if (t !== "" && Number.isFinite(f)) return Math.trunc(f); return $err("cannot convert \"" + t + "\" to int", l); },
  ParseFloat: (s, l) => { const t = s.trim(); const f = Number(t); if (t === "" || Number.isNaN(f)) $err("cannot convert \"" + t + "\" to float", l); return f; },
  IsIntStr: s => /^[+-]?\d+$/.test(s.trim()),
  ParseIntRadix: (s, r, l) => { const v = $R.$radixInt(s.trim(), $R.$radix(r, l)); return v === null ? $err("cannot convert \"" + s.trim() + "\" to an int in base " + r, l) : v; },
  IsIntRadix: (s, r, l) => $R.$radixInt(s.trim(), $R.$radix(r, l)) !== null,
  IntToStrRadix: (v, r, l) => v.toString($R.$radix(r, l)),
  $radix: (r, l) => (r >= 2 && r <= 36 ? r : $err("radix " + r + " is out of range, it must be between 2 and 36", l)),
  $radixInt: (t, r) => { const d = t.replace(/^[+-]/, ""); if (d === "" || [...d.toLowerCase()].some(c => { const n = parseInt(c, 36); return Number.isNaN(n) || n >= r; })) return null; const v = parseInt(t, r); return Number.isSafeInteger(v) ? v : null; },
  IsFloatStr: s => s.trim() !== "" && !Number.isNaN(Number(s.trim())),
  Print: s => { $write(s + "\n"); return 0; },
  PrintRaw: s => { $write(s); return 0; },
  Input: p => $readLine(p),
  PrintErr: s => { $flush(); process.stderr.write(s + "\n"); return 0; },
  ReadStdin: () => { $flush(); try { return $fs.readFileSync(0, "utf8"); } catch (e) { return ""; } },
  ArrLen: a => a.length,
  ArrGet: (a, i, l) => $idx(a, i, l),
  ArrSet: (a, i, v, l) => $sidx(a, i, v, l),
  ArrPush: (a, v) => { a.push(v); return 0; },
  ArrPop: (a, l) => { if (!a.length) $err("pop from empty array", l); return a.pop(); },
  ArrInsert: (a, i, v, l) => { if (i < 0 || i > a.length) $err("index " + i + " out of bounds (length " + a.length + ")", l); a.splice(i, 0, v); return 0; },
  ArrRemove: (a, i, l) => { if (i < 0 || i >= a.length) $err("index " + i + " out of bounds (length " + a.length + ")", l); return a.splice(i, 1)[0]; },
  ArrConcat: (a, b) => a.concat(b),
  ArrSlice: (a, s, e) => { const n = a.length; const end = Math.max(0, Math.min(e, n)); const st = Math.max(0, Math.min(s, end)); return a.slice(st, end); },
  ArrCopy: a => a.slice(),
  ArrIndexOf: (a, v, t) => a.findIndex(x => $eq(x, v, t)),
  ArrContains: (a, v, t) => a.some(x => $eq(x, v, t)),
  ArrJoin: (a, sep, t) => a.map(x => $fmt(x, t, false, 0)).join(sep),
  ArrReverse: a => { a.reverse(); return 0; },
  ArrSort: (a, t) => { a.sort((x, y) => $cmp(x, y, t)); return 0; },
  ArrClear: a => { a.length = 0; return 0; },
  MapNew: t => new $Map(t),
  MapSet: (m, k, v) => { m.m.set($mk(m, k), [k, v]); return v; },
  MapGet: (m, k, l) => { const e = m.m.get($mk(m, k)); if (!e) $err("key " + $fmt(k, $d(m.t)[1], true, 0) + " not found in map", l); return e[1]; },
  MapGetOr: (m, k, d) => { const e = m.m.get($mk(m, k)); return e ? e[1] : d; },
  MapFind: (m, k, t) => { const e = m.m.get($mk(m, k)); return e ? ($unboxed(t) ? $box(e[1], t) : e[1]) : null; },
  MapHas: (m, k) => m.m.has($mk(m, k)),
  MapRemove: (m, k) => m.m.delete($mk(m, k)),
  MapKeys: (m) => Array.from(m.m.values()).map(e => e[0]),
  MapValues: (m) => Array.from(m.m.values()).map(e => e[1]),
  MapLen: m => m.m.size,
  Box: (v, t) => $box(v, t),
  IsType: (v, f, t) => $isType(v, f, t),
  Destroy: (v, l) => { if (v === null) $err("cannot destroy null", l); if (!v.$rec) $err("only struct objects can be destroyed", l); if (v.$dead) $err("this object was already destroyed", l); v.$dead = true; for (let i = 1; i < v.length; i++) v[i] = null; return 0; },
  Alive: (v, l) => { if (v !== null && v.$dead) $err("this object was destroyed and can no longer be used", l); return v; },
  Cast: (v, f, t, l) => {
    if (!$isType(v, f, t)) {
      const fd = $d(f)[0];
      const actual = fd === "any" ? (v === null ? "null" : $tname(v.b)) : (fd === "opt" && v === null) ? "null" : (fd === "iface" || fd === "rec") ? $tname(v[0]) : $tname(f);
      $err("cannot cast value of type " + actual + " to " + $tname(t), l);
    }
    return $payload(v, f, t);
  },
  Unwrap: (v, t, l) => { if (v === null) $err("unexpected null value", l); const d = $d(t); return d[0] === "opt" && $unboxed(d[1]) ? v.v : v; },
  Eq: (a, b, t) => $eq(a, b, t),
  Cmp: (a, b, t) => $cmp(a, b, t),
  TypeName: (v, t) => { const d = $d(t); if (d[0] === "any") return v === null ? "null" : $tname(v.b); if (d[0] === "iface") return $tname(v[0]); if (d[0] === "opt") return v === null ? "null" : $tname(d[1]); return $tname(t); },
  AnyIndex: (v, k, kt, l) => {
    if (v === null) $err("cannot index null", l);
    const d = $d(v.b); const inner = v.v;
    if (d[0] === "map") { const kd = $d(d[1])[0]; let raw = kd === "any" ? $box(k, kt) : ($d(kt)[0] === "any" ? (k === null ? null : k.v) : k); const e = inner.m.get($mk(inner, raw)); return e ? $box(e[1], d[2]) : null; }
    if (d[0] === "arr") { const i = $d(kt)[0] === "int" ? k : (k && k.b === K.INT ? k.v : $err("array index must be an int", l)); return $box($idx(inner, i, l), d[1]); }
    if (d[0] === "rec") { const name = $d(kt)[0] === "str" ? k : (k && k.b === K.STR ? k.v : $err("record field name must be a string", l)); const i = d[2].findIndex(f => f[0] === name); return i < 0 ? null : $box(inner[i + 1], d[2][i][1]); }
    if (d[0] === "str") { return new $Box(K.STR, $R.StrIndex(inner, k, l)); }
    return $err("cannot index value of type " + $tname(v.b), l);
  },
  AnyLen: (v, l) => { if (v === null) return 0; const d = $d(v.b)[0]; if (d === "map") return v.v.m.size; if (d === "arr") return v.v.length; if (d === "str") return $chars(v.v).length; return $err("value of type " + $tname(v.b) + " has no length", l); },
  FMath: (op, x) => [Math.sqrt, Math.floor, Math.ceil, x => (x < 0 ? -Math.round(-x) : Math.round(x)), Math.abs, Math.sin, Math.cos, Math.tan, Math.log, Math.log10, Math.exp, Math.asin, Math.acos, Math.atan][op](x),
  FPow: (a, b) => Math.pow(a, b),
  FAtan2: (a, b) => Math.atan2(a, b),
  FMod: (a, b) => a % b,
  IPow: (a, b) => (b < 0 ? 0 : $ov(Math.pow(a, b), -1)),
  IAbs: a => $ov(Math.abs(a), -1),
  IMin: (a, b) => Math.min(a, b),
  IMax: (a, b) => Math.max(a, b),
  FMin: (a, b) => Math.min(a, b),
  FMax: (a, b) => Math.max(a, b),
  Random: () => $rand(),
  RandomInt: (a, b) => (b <= a ? a : a + Math.floor($rand() * (b - a + 1))),
  Seed: s => { $seed = (s >>> 0) || 1; return 0; },
  NowMs: () => Date.now(),
  NowSec: () => Date.now() / 1000,
  ClockNs: () => Number(process.hrtime.bigint() - $start),
  Sleep: ms => { $flush(); Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, Math.max(0, ms)); return 0; },
  LocalTime: () => $localTime(),
  HttpRequest: (m, url, body, headers) => {
    const args = ["-sS", "-i", "-X", m.toUpperCase()];
    if (!headers.some(h => h.toLowerCase().startsWith("user-agent:"))) args.push("-H", "User-Agent: BurnLang/2.0");
    for (const h of headers) args.push("-H", h);
    if (body.length) args.push("--data-binary", "@-");
    args.push(url);
    try { return $httpSplit($cp.execFileSync("curl", args, { input: body, maxBuffer: 1 << 28 }).toString("utf8")); }
    catch (e) { return ["0", String(e.message || e), ""]; }
  },
  JsonParse: (s, l) => { try { return $jsonToAny(JSON.parse(s)); } catch (e) { return $err("invalid JSON: " + e.message, l); } },
  JsonStringify: (v, t) => $toJson(v, t),
  ReadFile: (p, l) => { try { return $fs.readFileSync(p, "utf8"); } catch (e) { return $err("cannot read file \"" + p + "\": " + e.message, l); } },
  WriteFile: (p, c) => { try { $fs.writeFileSync(p, c); return true; } catch (e) { return false; } },
  AppendFile: (p, c) => { try { $fs.appendFileSync(p, c); return true; } catch (e) { return false; } },
  FileExists: p => $fs.existsSync(p),
  Env: n => process.env[n] || "",
  Args: () => $args.slice(),
  Exec: (p, a, c) => {
    if (!c) $flush();
    const r = $cp.spawnSync(p, a, { stdio: c ? ["inherit", "pipe", "pipe"] : "inherit", maxBuffer: 1 << 28 });
    if (r.error) return ["-1", "", String(r.error.message)];
    const code = String(r.status === null ? 1 : r.status);
    return c ? [code, r.stdout.toString("utf8"), r.stderr.toString("utf8")] : [code, "", ""];
  },
  FsOp: (op, a, b) => {
    try {
      switch (op) {
        case "mkdir": $fs.mkdirSync(a, { recursive: true }); return true;
        case "remove": $fs.rmSync(a, { recursive: true, force: true }); return true;
        case "rename": $fs.renameSync(a, b); return true;
        case "copy": $fs.copyFileSync(a, b); return true;
        case "isDir": return $fs.statSync(a).isDirectory();
        case "isFile": return $fs.statSync(a).isFile();
        case "chdir": process.chdir(a); return true;
        default: return false;
      }
    } catch (e) { return false; }
  },
  ListDir: p => { try { return $fs.readdirSync(p).sort(); } catch (e) { return []; } },
  Cwd: () => process.cwd(),
  ExitNow: c => { $flush(); throw new BurnExit(c); },
  Panic: (m, l) => $err(m, l),
  Assert: (c, m, l) => { if (!c) $err("assertion failed: " + m, l); return 0; },
  ErrIndex: (l, i, n) => $err("index " + i + " out of bounds (length " + n + ")", l),
  ErrDivZero: l => $err("division by zero", l),
  Fail: (m, l) => $err(m, l),
  ErrOverflow: l => $err("integer overflow", l),
  NumFit: (v, c, l) => { const n = $NUMS[c]; if (v < n[1] || v > n[2]) $err("integer overflow: " + v + " does not fit in " + n[0], l); return v; },
  NumConv: (v, c, l) => { const k = c & 15; const n = k === 0 ? ["int", -9223372036854775808, 9223372036854775807] : $NUMS[k]; if (v < n[1] || v > n[2]) $err("cannot convert " + v + " to " + n[0], l); return v; },
  NumWrap: (v, c) => $nwrap(v, c),
  UAdd: (a, b, l) => { const r = a + b; if (r > 18446744073709551615) $err("integer overflow: " + a + " + " + b + " does not fit in uint64", l); return r; },
  USub: (a, b, l) => { if (b > a) $err("integer overflow: " + a + " - " + b + " does not fit in uint64", l); return a - b; },
  UMul: (a, b, l) => { const r = a * b; if (r > 18446744073709551615) $err("integer overflow: " + a + " * " + b + " does not fit in uint64", l); return r; },
  UDiv: (a, b, l) => { if (b === 0) $err("division by zero", l); return Math.trunc(a / b); },
  UMod: (a, b, l) => { if (b === 0) $err("division by zero", l); return a % b; },
  U2F: v => v,
  F2Num: (v, c, l) => { const k = c & 15; const n = k === 0 ? ["int", -9223372036854775808, 9223372036854775807] : $NUMS[k]; const t = Math.trunc(v); if (Number.isNaN(v) || t < n[1] || t > n[2]) $err("cannot convert " + $fstr(v) + " to " + n[0], l); return t; },
  F32Round: v => Math.fround(v),
  ShiftCheck: (b, l) => { if (b < 0 || b > 63) $err("cannot shift by " + b, l); return b; },
  ErrShift: (l, b) => $err("cannot shift by " + b, l),
  ErrNull: l => $err("unexpected null value", l),
  ErrReturn: l => $err("function ended without returning a value", l),
  Await: f => f.v,
  GcCollect: () => 0,
};
