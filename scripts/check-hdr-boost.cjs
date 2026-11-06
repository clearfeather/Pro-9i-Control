// Windows GUI executables need explicit inherited pipes to retain CLI output.
// Run after building: node scripts/check-hdr-boost.cjs [live|idle|battery|saver|power-transition|display-change|suspend|emergency|failure|panic|stall|capture-stall|diagnose|restore|preview]
const { spawn } = require("node:child_process");
const path = require("node:path");
const scenario = process.argv[2] || "live";
const args = scenario === "preview" ? ["preview"] : scenario === "diagnose" ? ["boost-diagnose"] : scenario === "restore" ? ["boost-restore"] : ["boost-test", "--scenario", scenario];
const executable = path.resolve(__dirname, "../src-tauri/target/release/HZHDR.exe");
const child = spawn(executable, args, { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
child.stdout.pipe(process.stdout);
child.stderr.pipe(process.stderr);
child.on("error", error => { console.error(error); process.exitCode = 1; });
child.on("exit", code => { process.exitCode = code ?? 1; });
