import { api } from "@/services/api";

/** A CSS color as `#rrggbb`, through a canvas: the theme's colors are written in oklch. */
function hex(color: string): string | null {
  const context = document.createElement("canvas").getContext("2d");
  if (!context) return null;
  context.fillStyle = color;
  context.fillRect(0, 0, 1, 1);
  const [r, g, b] = context.getImageData(0, 0, 1, 1).data;
  return `#${[r, g, b].map((part) => part.toString(16).padStart(2, "0")).join("")}`;
}

/**
 * Gives Android's status and navigation bars the color of what the app shows against them:
 * the top and bottom bars' (`--card`) once logged in, the page's (`--background`) before.
 * Other platforms ignore it.
 */
export function paintSystemBars(surface: "card" | "background") {
  const root = document.documentElement;
  const color = hex(getComputedStyle(root).getPropertyValue(`--${surface}`).trim());
  if (color) api.setBarsColor(color, root.classList.contains("dark")).catch(() => {});
}
