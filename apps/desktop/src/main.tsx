import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [v, setV] = useState("…");
  useEffect(() => {
    invoke<string>("core_version").then(setV).catch(() => setV("web"));
  }, []);
  return <main style={{ fontFamily: "system-ui", padding: 32 }}>Mocó · core {v}</main>;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
