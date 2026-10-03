import { mount } from "svelte";
import { Toaster } from "@/components/ui/sonner";
import App from "./App.svelte";
import "./styles.css";

async function main() {
  // The web version has no Tauri: it runs the Rust core in a Web Worker behind a stand-in for
  // Tauri's IPC. Only the web build (`--mode web`) includes it.
  if (import.meta.env.MODE === "web") {
    const { installWebBackend } = await import("./web/install");
    if (!(await installWebBackend())) return;
  }

  const target = document.getElementById("root");
  if (target) {
    // Replaces the static splash from index.html, which App renders identically until it knows
    // the account.
    target.replaceChildren();
    mount(App, { target });
    // Outside the app's root, which is hidden from screen readers while a dialog is open, so
    // toasts are still announced.
    mount(Toaster, { target: document.body, props: { position: "top-center" } });
  }
}

main();
