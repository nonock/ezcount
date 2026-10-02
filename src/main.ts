import { mount } from "svelte";
import { Toaster } from "@/components/ui/sonner";
import App from "./App.svelte";
import "./styles.css";

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
