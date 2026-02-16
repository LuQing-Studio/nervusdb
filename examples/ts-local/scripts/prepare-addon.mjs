import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "../../..");
const releaseDir = path.join(repoRoot, "nervusdb-node", "target", "release");
const nativeDir = path.resolve(__dirname, "..", "native");
const targetAddon = path.join(nativeDir, "nervusdb_node.node");

const candidates = [
  path.join(releaseDir, "libnervusdb_node.dylib"),
  path.join(releaseDir, "libnervusdb_node.so"),
  path.join(releaseDir, "libnervusdb_node.dll"),
  path.join(releaseDir, "nervusdb_node.dll")
];

const sourceAddon = candidates.find((candidate) => fs.existsSync(candidate));
if (!sourceAddon) {
  console.error("[ts-local] failed: Node addon artifact not found.");
  console.error("[ts-local] run: cargo build --manifest-path nervusdb-node/Cargo.toml --release");
  process.exit(2);
}

fs.mkdirSync(nativeDir, { recursive: true });
fs.copyFileSync(sourceAddon, targetAddon);
console.log(`[ts-local] prepared addon: ${sourceAddon} -> ${targetAddon}`);
