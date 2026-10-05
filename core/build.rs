//! Hands the core the app's version, as `EZCOUNT_APP_VERSION`. The core has none of its own:
//! the app carries it, in `package.json` like everywhere a release sets it
//! (`scripts/release.ts`), and the core says it to the relay (`sync::APP_VERSION`).

use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../package.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", manifest.display()));
    let version = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("\"version\": \""))
        .and_then(|rest| rest.split('"').next())
        .filter(|version| {
            // major.minor.patch, as the relay reads it.
            version.split('.').count() == 3
                && version
                    .split('.')
                    .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .unwrap_or_else(|| panic!("no \"version\": \"x.y.z\" in {}", manifest.display()));
    println!("cargo:rustc-env=EZCOUNT_APP_VERSION={version}");
}
