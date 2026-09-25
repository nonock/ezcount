fn main() {
    // The Windows .exe embeds icons/icon.ico as a resource, and the app loads its window icons
    // from it: rebuild the resource whenever the icon changes, not only on a clean build.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build()
}
