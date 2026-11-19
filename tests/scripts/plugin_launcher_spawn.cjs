// Start the plugin's MCP server the way Claude Code does, and check that it answers.
//
// Claude Code starts a stdio MCP server through the MCP SDK's
// StdioClientTransport: cross-spawn with shell: false. On Windows, cross-spawn
// resolves the extensionless command in plugin/.mcp.json to act-lite-mcp.cmd.
//
//   CROSS_SPAWN=<path to cross-spawn> node plugin_launcher_spawn.cjs
//
// The environment (CLAUDE_PLUGIN_DATA, ACT_LITE_BASE_URL, PATH) passes through.
// Exit 0 when the server answers initialize as act-lite; 1 otherwise.

const path = require("path");
const fs = require("fs");
const spawn = require(process.env.CROSS_SPAWN || "cross-spawn");

const plugin = path.resolve(__dirname, "../../plugin");
const server = JSON.parse(fs.readFileSync(path.join(plugin, ".mcp.json"), "utf8")).mcpServers["act-lite"];
const command = server.command.replace("${CLAUDE_PLUGIN_ROOT}", plugin);

const child = spawn(command, server.args, {
  env: process.env,
  stdio: ["pipe", "pipe", "pipe"],
  shell: false,
  windowsHide: process.platform === "win32",
});

let stdout = "";
let stderr = "";
const finish = (ok, why) => {
  if (!ok) {
    console.error(`FAIL: ${why}\n--- stdout\n${stdout}\n--- stderr\n${stderr}`);
  } else {
    console.log(`OK: ${why}`);
  }
  child.stdin.end();
  child.kill();
  process.exit(ok ? 0 : 1);
};

const timer = setTimeout(() => finish(false, "no answer within 60 s"), 60000);
child.on("error", (e) => finish(false, `spawn error: ${e.message}`));
child.on("exit", (code) => finish(false, `server exited with ${code} before answering`));
child.stderr.on("data", (d) => (stderr += d));
child.stdout.on("data", (d) => {
  stdout += d;
  const line = stdout.split("\n")[0];
  if (!stdout.includes("\n")) return;
  clearTimeout(timer);
  let reply;
  try {
    reply = JSON.parse(line);
  } catch {
    return finish(false, "the first stdout line is not JSON");
  }
  const name = reply.result && reply.result.serverInfo && reply.result.serverInfo.name;
  finish(name === "act-lite", `serverInfo.name is ${name}`);
});

child.stdin.write(
  JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "launcher-check", version: "0" } },
  }) + "\n"
);
