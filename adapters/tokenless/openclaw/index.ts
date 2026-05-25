/**
 * Token-Less Unified Plugin for OpenClaw v5
 *
 * Strategies: RTK command rewriting, response compression, TOON encoding, Tool Ready.
 */

import { execFileSync, spawnSync } from "child_process";
import { existsSync, statSync } from "fs";

// ---- Binary cache ----------------------------------------------------------------

let rtkAvailable: boolean | null = null;
let tokenlessAvailable: boolean | null = null;
let rtkPath = "rtk";
let tokenlessPath = "tokenless";

function isExecutable(path: string): boolean {
  try {
    return existsSync(path) && (statSync(path).mode & 0o111) !== 0;
  } catch {
    return false;
  }
}

function isSkillContent(msg: any): boolean {
  if (typeof msg !== "string") return false;
  const t = msg.trimStart();
  if (!t.startsWith("---")) return false;
  return /^name:/m.test(t) || /^description:/m.test(t);
}

function checkRtk(): boolean {
  if (rtkAvailable !== null) return rtkAvailable;
  try {
    const r = execFileSync("which", ["rtk"], { encoding: "utf-8", timeout: 2000 }).trim();
    if (r) { rtkPath = r; rtkAvailable = true; return true; }
  } catch { /* not on PATH */ }
  rtkAvailable = false;
  return false;
}

function checkTokenless(): boolean {
  if (tokenlessAvailable !== null) return tokenlessAvailable;
  try {
    const r = execFileSync("which", ["tokenless"], { encoding: "utf-8", timeout: 2000 }).trim();
    if (r) { tokenlessPath = r; tokenlessAvailable = true; return true; }
  } catch { /* not on PATH */ }
  tokenlessAvailable = false;
  return false;
}

// ---- Helpers --------------------------------------------------------------------

function tryRtkRewrite(cmd: string): string | null {
  try {
    const r = spawnSync(rtkPath, ["rewrite", cmd], { encoding: "utf-8", timeout: 2000 });
    const out = r.stdout?.trim();
    if ((r.status === 0 || r.status === 3) && out && out !== cmd) return out;
  } catch { /* ignore */ }
  return null;
}

function tryCompressResponse(data: any, sessionId?: string, toolCallId?: string): any | null {
  try {
    const input = JSON.stringify(data);
    const args = ["compress-response", "--agent-id", "openclaw"];
    if (sessionId) args.push("--session-id", sessionId);
    if (toolCallId) args.push("--tool-use-id", toolCallId);
    const r = execFileSync(tokenlessPath, args, { encoding: "utf-8", timeout: 3000, input }).trim();
    if (r === input) return null;
    return JSON.parse(r);
  } catch { return null; }
}

function tryCompressToon(data: any): { text: string; pct: number } | null {
  try {
    const input = JSON.stringify(data);
    const args = ["compress-toon", "--agent-id", "openclaw"];
    const r = execFileSync(tokenlessPath, args, { encoding: "utf-8", timeout: 3000, input }).trim();
    if (!r || r === input || r.length > input.length) return null;
    return { text: r, pct: Math.round(((input.length - r.length) / input.length) * 100) };
  } catch { return null; }
}

function tryEnvCheck(tool: string): { status: string; diagnostic: string } | null {
  try {
    const out = execFileSync(tokenlessPath, ["env-check", "--tool", tool, "--json"], { encoding: "utf-8", timeout: 3000 }).trim();
    const s = JSON.parse(out);
    if (s.status === "UNKNOWN" || s.status === "READY") return null;
    const fix = execFileSync(tokenlessPath, ["env-check", "--tool", tool, "--fix", "--json"], { encoding: "utf-8", timeout: 10000 }).trim();
    const fp = JSON.parse(fix);
    if (fp.status === "READY") return null;
    return { status: fp.status, diagnostic: fp.diagnostic || `[tokenless tool-ready] ${tool}: NOT_READY` };
  } catch { return null; }
}

// ---- Plugin entry ---------------------------------------------------------------

export default {
  id: "tokenless-openclaw",
  name: "Token-Less",
  version: "1.0.0",
  description: "RTK rewriting + response compression + TOON + Tool Ready",
  register(api: any) {
    const cfg = api.config ?? {};
    const rtkOk = cfg.rtk_enabled !== false && checkRtk();
    const tlOk = checkTokenless();
    const respOk = cfg.response_compression_enabled !== false && tlOk;
    const toonOk = cfg.toon_compression_enabled === true && tlOk;
    const trOk = cfg.tool_ready_enabled !== false && tlOk;
    const verbose = cfg.verbose !== false;
    const skipTools: Set<string> = new Set(
      (cfg.skip_tools ?? ["Read", "read_file", "Glob", "list_directory", "NotebookRead"]).map((t: string) => t.toLowerCase())
    );

    // Tool Ready
    if (trOk) {
      api.on("before_tool_call", (ev: any, ctx: any) => {
        const r = tryEnvCheck(ev.toolName);
        if (!r) return;
        if (verbose) console.log(`[tokenless/tool-ready] ${ev.toolName}: ${r.status}`);
        return { contextPrefix: r.diagnostic };
      }, { priority: 5 });
    }

    // RTK rewrite
    if (rtkOk) {
      api.on("before_tool_call", (ev: any, ctx: any) => {
        if (ev.toolName !== "exec") return;
        const cmd = ev.params?.command;
        if (typeof cmd !== "string") return;
        process.env.TOKENLESS_AGENT_ID = "openclaw";
        if (ctx?.sessionId) process.env.TOKENLESS_SESSION_ID = ctx.sessionId;
        if (ctx?.toolCallId) process.env.TOKENLESS_TOOL_USE_ID = ctx.toolCallId;
        const r = tryRtkRewrite(cmd);
        if (!r) return;
        if (verbose) console.log(`[tokenless/rtk] ${cmd} -> ${r}`);
        return { params: { ...ev.params, command: r } };
      }, { priority: 10 });
    }

    // Response / TOON compression
    if (tlOk && (respOk || toonOk)) {
      api.on("tool_result_persist", (ev: any, ctx: any) => {
        const before = JSON.stringify(ev.message);
        if (before.length < 200) return;
        if (ev.toolName && skipTools.has(ev.toolName.toLowerCase())) return;
        if (isSkillContent(ev.message)) return;
        const tid = ctx?.toolCallId || ev.toolCallId;
        const sid = ctx?.sessionId || process.env.TOKENLESS_SESSION_ID || ctx?.sessionKey;

        let current = ev.message;
        let usedResp = false;
        if (respOk) {
          const c = tryCompressResponse(current, sid, tid);
          if (c) { current = c; usedResp = true; }
        }

        let usedToon = false;
        let toonText = "";
        if (toonOk) {
          const t = tryCompressToon(current);
          if (t) { toonText = t.text; usedToon = true; }
        }
        if (!usedResp && !usedToon) return;

        if (usedToon) {
          const pct = Math.round(((JSON.stringify(ev.message).length - toonText.length) / JSON.stringify(ev.message).length) * 100);
          const label = usedResp ? "response compressed + TOON encoded" : "TOON encoded";
          const wrapped = `[TOON format, ${pct}% token savings]\n${toonText}`;
          if (typeof ev.message === "object" && ev.message?.role === "toolResult") {
            current = { ...ev.message, content: [{ type: "text", text: wrapped }] };
          } else { current = wrapped; }
        } else {
          current = current;
        }

        if (verbose) {
          const aft = usedToon ? toonText.length : JSON.stringify(current).length;
          console.log(`[tokenless/${usedToon ? "TOON" : "response"}] ${ev.toolName}: ${before.length} -> ${aft} chars`);
        }
        return { message: current };
      }, { priority: 10 });
    }

    if (verbose) {
      const feats = [rtkOk && "rtk-rewrite", trOk && "tool-ready", respOk && "response-compression", toonOk && "toon-compression"].filter(Boolean);
      console.log(`[tokenless] plugin registered — ${feats.join(", ") || "no features"}`);
    }
  },
};
