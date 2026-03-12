#!/usr/bin/env node
/**
 * Usage: pnpm bump <patch|minor|major|x.y.z>
 *
 * Bumps the version in package.json and Cargo.toml, commits, and tags.
 * tauri.conf.json reads from package.json automatically.
 */
import { readFileSync, writeFileSync } from "fs";
import { execSync } from "child_process";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

const arg = process.argv[2];

if (!arg) {
  console.error("Usage: pnpm bump <patch|minor|major|x.y.z>");
  process.exit(1);
}

// Read current version from package.json
const pkgPath = resolve(root, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
const current = pkg.version;

// Compute new version
let next;

if (["patch", "minor", "major"].includes(arg)) {
  const [major, minor, patch] = current.split(".").map(Number);

  if (arg === "patch") {
    next = `${major}.${minor}.${patch + 1}`;
  }

  if (arg === "minor") {
    next = `${major}.${minor + 1}.0`;
  }

  if (arg === "major") {
    next = `${major + 1}.0.0`;
  }
} else if (/^\d+\.\d+\.\d+$/.test(arg)) {
  next = arg;
} else {
  console.error(`Invalid version argument: ${arg}`);
  process.exit(1);
}

// Update package.json
pkg.version = next;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

// Update Cargo.toml
const cargoPath = resolve(root, "src-tauri", "Cargo.toml");
let cargo = readFileSync(cargoPath, "utf-8");
cargo = cargo.replace(/^version\s*=\s*"[^"]*"/m, `version = "${next}"`);
writeFileSync(cargoPath, cargo);

console.log(`${current} → ${next}`);
console.log("Updated: package.json, Cargo.toml");
console.log("tauri.conf.json reads from package.json automatically.");

// Stage, commit, tag
execSync(`git add "${pkgPath}" "${cargoPath}"`, {
  cwd: root,
  stdio: "inherit",
});
execSync(`git commit -m "v${next}"`, { cwd: root, stdio: "inherit" });
execSync(`git tag v${next}`, { cwd: root, stdio: "inherit" });
console.log(
  `\nTagged v${next}. Push with: git push && git push origin v${next}`,
);
