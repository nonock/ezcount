// Applies the saved theme (mode-watcher, "theme" key) before the first paint, so the splash
// and the app never flash the wrong background. A file rather than inline, so the web
// version's Content-Security-Policy can refuse inline scripts.
(() => {
  try {
    const t = localStorage.getItem("theme");
    const dark =
      t === "dark" ||
      ((!t || t === "system") && matchMedia("(prefers-color-scheme: dark)").matches);
    const root = document.documentElement;
    root.classList.add(dark ? "dark" : "light");
    root.style.colorScheme = dark ? "dark" : "light";
  } catch {
    // No saved theme: the app's default (light) shows until it starts.
  }
})();
