// Builds the web version's Rust core (`web/`) to WebAssembly, with its JavaScript glue in
// `web/pkg/` (imported as `ezcount-web`, see vite.config.ts).
//
//   bun scripts/build-wasm.ts           # optimized
//   bun scripts/build-wasm.ts --dev     # faster build, slower code
//   bun scripts/build-wasm.ts --lint    # clippy on the core and web crates, for WebAssembly
//
// Needs the `wasm32-unknown-unknown` Rust target, the wasm-bindgen CLI at the version in
// web/Cargo.lock, and a clang that targets WebAssembly to compile SQLite: the one on PATH, or
// on Windows the Android NDK's (NDK_HOME).
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const dev = process.argv.includes("--dev");
const target = "wasm32-unknown-unknown";

function fail(message: string): never {
  console.error(`build-wasm: ${message}`);
  process.exit(1);
}

function run(command: string, args: string[], env: NodeJS.ProcessEnv = process.env) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: "inherit", shell: false });
  if (result.error) fail(`could not run ${command}: ${result.error.message}`);
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function output(command: string, args: string[]): string | null {
  const result = spawnSync(command, args, { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : null;
}

/** A tool on PATH, or the first of `candidates` that exists. */
function findTool(names: string[], candidates: string[]): string | null {
  for (const name of names) {
    if (output(name, ["--version"]) !== null) return name;
  }
  return candidates.find((path) => existsSync(path)) ?? null;
}

/**
 * The Android NDK: NDK_HOME, also when this process started before it was set (VS Code and
 * its git hooks, say). Windows keeps user variables in the registry.
 */
function ndkHome(): string | undefined {
  if (process.env.NDK_HOME) return process.env.NDK_HOME;
  if (process.platform !== "win32") return undefined;
  const query = output("reg", ["query", "HKCU\\Environment", "/v", "NDK_HOME"]);
  const value = query?.match(/NDK_HOME\s+REG_(?:EXPAND_)?SZ\s+(.+)/)?.[1]?.trim();
  return value?.replace(/%([^%]+)%/g, (whole, name) => process.env[name] ?? whole);
}

// SQLite is C, compiled for WebAssembly by cc-rs: it needs clang and llvm-ar.
const env = { ...process.env };
if (!env.CC_wasm32_unknown_unknown) {
  const ndk = ndkHome();
  const ndkBin = ndk ? join(ndk, "toolchains", "llvm", "prebuilt", "windows-x86_64", "bin") : "";
  const clang = findTool(["clang"], ndk ? [join(ndkBin, "clang.exe")] : []);
  if (!clang) fail("no clang found: install clang (LLVM), or set NDK_HOME to an Android NDK");
  const versions = [22, 21, 20, 19, 18, 17, 16, 15].map((v) => `llvm-ar-${v}`);
  const ar = findTool(["llvm-ar", ...versions], ndk ? [join(ndkBin, "llvm-ar.exe")] : []);
  if (!ar) fail("no llvm-ar found: install LLVM, or set NDK_HOME to an Android NDK");
  env.CC_wasm32_unknown_unknown = clang;
  env.AR_wasm32_unknown_unknown = ar;
}

if (process.argv.includes("--lint")) {
  // The code behind `cfg(target_family = "wasm")`, which native builds never see.
  for (const crate of ["core", "web"]) {
    run(
      "cargo",
      [
        "clippy",
        "--locked",
        "--manifest-path",
        `${crate}/Cargo.toml`,
        "--target",
        target,
        "--",
        "-D",
        "warnings",
      ],
      env
    );
  }
  process.exit(0);
}

const profile = dev ? "debug" : "release";
run(
  "cargo",
  [
    "build",
    "--locked",
    "--manifest-path",
    "web/Cargo.toml",
    "--target",
    target,
    ...(dev ? [] : ["--release"]),
  ],
  env
);

// The CLI must match the wasm-bindgen crate exactly, or the glue doesn't fit the module.
const lock = readFileSync(join(root, "web", "Cargo.lock"), "utf8");
const wanted = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/)?.[1];
const installed = output("wasm-bindgen", ["--version"])?.split(" ")[1];
if (installed !== wanted) {
  fail(
    `wasm-bindgen ${wanted} is needed (found ${installed ?? "none"}): ` +
      `cargo install wasm-bindgen-cli --version ${wanted} --locked`
  );
}
run("wasm-bindgen", [
  "--target",
  "web",
  "--no-typescript",
  "--out-dir",
  "web/pkg",
  join("web", "target", target, profile, "ezcount_web.wasm"),
]);
