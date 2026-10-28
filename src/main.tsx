import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// macOS windows are Liquid Glass / vibrancy (tauri.macos.conf.json); other platforms get
// an opaque window, so only let the glass show through on the Mac.
if (navigator.userAgent.includes("Mac")) document.documentElement.classList.add("native-glass");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
