#!/usr/bin/env node
/**
 * Fail fast when Node is below the version required by package.json engines.
 * Avoids opaque errors from Vite 7 (e.g. crypto.hash is not a function on Node 18).
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(
  readFileSync(join(__dirname, "..", "package.json"), "utf8"),
);
const range = pkg.engines?.node;
if (!range) process.exit(0);

const major = Number.parseInt(process.versions.node, 10);
if (Number.isNaN(major) || major < 24) {
  console.error(
    `\x1b[31mThis project requires Node.js 24.x (see .node-version and package.json → engines: "${range}").\x1b[0m`,
  );
  console.error(`You are running Node ${process.version}.`);
  console.error(
    "\nFix: install Node 24, then from the repo root run something like:",
  );
  console.error("  fnm use          # or: nvm use");
  console.error("  node -v          # should show v24.x");
  console.error("  npm install");
  console.error("  npm run tauri dev\n");
  process.exit(1);
}
