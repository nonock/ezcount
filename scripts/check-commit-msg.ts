// commit-msg hook: requires a Conventional Commits subject, e.g. `feat(ui): add dark mode`.
// Usage: bun scripts/check-commit-msg.ts <path to commit message file>
import { readFileSync } from "node:fs";

const TYPES = [
  "feat",
  "fix",
  "perf",
  "refactor",
  "docs",
  "test",
  "style",
  "build",
  "ci",
  "chore",
  "revert",
];
const PATTERN = new RegExp(String.raw`^(${TYPES.join("|")})(\([\w./-]+\))?!?: \S`);
// Messages git writes itself, and fixups that get squashed away.
const EXEMPT = /^(Merge |Revert "|fixup! |squash! |amend! )/;

const file = process.argv[2];
if (!file) {
  console.error("check-commit-msg: missing commit message file argument");
  process.exit(2);
}

const subject =
  readFileSync(file, "utf8")
    .split(/\r?\n/)
    .find((line) => line.trim() !== "" && !line.startsWith("#")) ?? "";

if (!EXEMPT.test(subject) && !PATTERN.test(subject)) {
  console.error(`Commit subject is not a Conventional Commit:\n\n  ${subject || "(empty)"}\n`);
  console.error("Expected `<type>(<optional scope>): <description>`, for example:");
  console.error("  feat(sync): retry failed pushes");
  console.error("  fix: keep the form open during a background sync\n");
  console.error(`Types: ${TYPES.join(", ")}`);
  process.exit(1);
}
