import { ConfirmProvider } from "@/components/common/ConfirmDialog";
import { Toaster } from "@/components/ui/sonner";
import { ThemeProvider } from "next-themes";
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const rootElement = document.getElementById("root");
if (rootElement) {
  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
        <ConfirmProvider>
          <App />
          <Toaster position="top-center" />
        </ConfirmProvider>
      </ThemeProvider>
    </React.StrictMode>
  );
}
