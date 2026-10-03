// Prepares a release: sets the version everywhere, commits and tags. Pushing the tag starts
// .github/workflows/release.yml (packages, relay deploy, GitHub release).
//
//   bun run release 0.2.0          bump, date the changelog, commit "chore(release): v0.2.0", tag v0.2.0
//   bun run release --check v0.2.0 CI: fail unless every file says 0.2.0
//   bun run release --notes v0.2.0 CI: print that version's section of CHANGELOG.md
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

const CHANGELOG = "CHANGELOG.md";
const UNRELEASED = "## Unreleased";
/** The text under a changelog heading, up to the next version's. */
function changelogSection(heading: string): string | undefined {
  const lines = read(CHANGELOG).replaceAll("\r\n", "\n").split("\n");
  const start = lines.findIndex((line) => line === heading || line.startsWith(`${heading} `));
  if (start < 0) return undefined;
  const length = lines.slice(start + 1).findIndex((line) => line.startsWith("## "));
  return lines
    .slice(start + 1, length < 0 ? undefined : start + 1 + length)
    .join("\n")
    .trim();
}

const [first, second] = process.argv.slice(2);

if (first === "--notes") {
  const notes = changelogSection(`## ${second?.replace(/^v/, "")}`);
  if (!notes) {
    console.error(`${CHANGELOG} has nothing for ${second}.`);
    process.exit(1);
  }
  console.log(notes);
  process.exit(0);
}

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
if (!changelogSection(UNRELEASED)) {
  fail(`Say what changed under "${UNRELEASED}" in ${CHANGELOG} first.`);
}

// What was unreleased becomes this version's section, under a new empty one.
const today = new Date().toISOString().slice(0, 10);
writeFileSync(
  new URL(CHANGELOG, root),
  read(CHANGELOG).replace(UNRELEASED, `${UNRELEASED}\n\n## ${version} - ${today}`)
);

for (const [path, pattern] of files) {
  writeFileSync(new URL(path, root), read(path).replace(pattern, `$1${version}$3`));
}
// Record the crates' new versions in their lock files (and nothing else). The core's lists the
// relay too: its tests run one.
for (const manifest of ["src-tauri/Cargo.toml", "sync-server/Cargo.toml"]) {
  run("cargo", "update", "--quiet", "--workspace", "--manifest-path", manifest);
}
run(
  "cargo",
  "update",
  "--quiet",
  "-p",
  "ezcount-sync-server",
  "--manifest-path",
  "core/Cargo.toml"
);

run(
  "git",
  "add",
  ...files.map(([path]) => path),
  CHANGELOG,
  "src-tauri/Cargo.lock",
  "sync-server/Cargo.lock",
  "core/Cargo.lock"
);
execFileSync("git", ["commit", "-m", `chore(release): ${tag}`], { cwd: root, stdio: "inherit" });
run("git", "tag", "-a", tag, "-m", `ezcount ${version}`);
console.log(`\nTagged ${tag}. Publish it with:\n  git push --follow-tags`);
