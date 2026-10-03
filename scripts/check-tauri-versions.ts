// Fails when the Rust and JavaScript sides of Tauri are on different minor versions.
// The Tauri CLI refuses to build in CI in that case (`tauri` vs `@tauri-apps/api`, and each
// `tauri-plugin-x` crate vs `@tauri-apps/plugin-x`), so this catches it before a slow build.
// Reads the lock files, so it checks exactly what gets built.
import { readFileSync } from "node:fs";

const root = new URL("..", import.meta.url);
const cargoLock = readFileSync(new URL("src-tauri/Cargo.lock", root), "utf8");
const bunLock = readFileSync(new URL("bun.lock", root), "utf8");

const crates = new Map<string, string>();
for (const [, name, version] of cargoLock.matchAll(/\nname = "([^"]+)"\nversion = "([^"]+)"/g)) {
  crates.set(name, version);
}

const packages = new Map<string, string>();
for (const [, name, version] of bunLock.matchAll(
  /"(@tauri-apps\/[\w-]+)": \["@tauri-apps\/[\w-]+@([^"]+)"/g
)) {
  packages.set(name, version);
}

const pairs: [crate: string, pkg: string][] = [["tauri", "@tauri-apps/api"]];
for (const pkg of packages.keys()) {
  const plugin = pkg.match(/^@tauri-apps\/plugin-(.+)$/)?.[1];
  if (plugin) pairs.push([`tauri-plugin-${plugin}`, pkg]);
}

const minor = (version: string) => version.split(".").slice(0, 2).join(".");
let failed = false;
for (const [crate, pkg] of pairs) {
  const rust = crates.get(crate);
  const js = packages.get(pkg);
  if (!rust || !js) {
    console.error(
      `missing: ${crate} ${rust ?? "(not in Cargo.lock)"}, ${pkg} ${js ?? "(not in bun.lock)"}`
    );
    failed = true;
    continue;
  }
  const ok = minor(rust) === minor(js);
  failed ||= !ok;
  console.log(`${ok ? "ok      " : "MISMATCH"} ${crate} ${rust}  <->  ${pkg} ${js}`);
}

if (failed) {
  console.error(
    "\nTauri's Rust crates and npm packages must share major.minor versions. Upgrade both sides" +
      ' together (see "Upgrading Tauri" in docs/development.md).'
  );
  process.exit(1);
}
