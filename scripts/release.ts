// Prepares a release: sets the version everywhere, commits and tags. Pushing the tag starts
// .github/workflows/release.yml (packages, relay deploy, GitHub release).
//
//   bun run release 0.2.0          bump, commit "chore(release): v0.2.0", tag v0.2.0
//   bun run release --check v0.2.0 CI: fail unless every file says 0.2.0
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const root = new URL("..", import.meta.url);
const run = (cmd: string, ...args: string[]) =>
  execFileSync(cmd, args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });

// Each file holding the version, with the pattern that finds it (first match only).
const files: [path: string, pattern: RegExp][] = [
  ["package.json", /("version": ")([^"]+)(")/],
  ["src-tauri/tauri.conf.json", /("version": ")([^"]+)(")/],
  ["src-tauri/Cargo.toml", /(\[package\][^[]*?\nversion = ")([^"]+)(")/],
  ["sync-server/Cargo.toml", /(\[package\][^[]*?\nversion = ")([^"]+)(")/],
];
const read = (path: string) => readFileSync(new URL(path, root), "utf8");

const [first, second] = process.argv.slice(2);

if (first === "--check") {
  const expected = second?.replace(/^v/, "");
  let failed = !expected;
  for (const [path, pattern] of files) {
    const version = read(path).match(pattern)?.[2];
    const ok = version === expected;
    failed ||= !ok;
    console.log(`${ok ? "ok      " : "MISMATCH"} ${path} ${version ?? "(no version found)"}`);
  }
  if (failed) {
    console.error(
      `\nTag ${second} doesn't match. Create releases with \`bun run release <version>\`.`
    );
    process.exit(1);
  }
  process.exit(0);
}

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

const version = first?.replace(/^v/, "");
// Android derives its versionCode from major.minor.patch, so no pre-release suffixes.
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  fail("usage: bun run release <major.minor.patch>");
}
if (run("git", "status", "--porcelain").trim()) fail("Commit or stash your changes first.");
const branch = run("git", "branch", "--show-current").trim();
if (branch !== "main") fail(`Releases are made from main, not ${branch}.`);
const tag = `v${version}`;
if (run("git", "tag", "--list", tag).trim()) fail(`${tag} already exists.`);

for (const [path, pattern] of files) {
  writeFileSync(new URL(path, root), read(path).replace(pattern, `$1${version}$3`));
}
// Record the crates' new versions in their lock files (and nothing else).
for (const manifest of ["src-tauri/Cargo.toml", "sync-server/Cargo.toml"]) {
  run("cargo", "update", "--quiet", "--workspace", "--manifest-path", manifest);
}

run("git", "add", ...files.map(([path]) => path), "src-tauri/Cargo.lock", "sync-server/Cargo.lock");
execFileSync("git", ["commit", "-m", `chore(release): ${tag}`], { cwd: root, stdio: "inherit" });
run("git", "tag", "-a", tag, "-m", `ezcount ${version}`);
console.log(`\nTagged ${tag}. Publish it with:\n  git push --follow-tags`);
