import { useState } from "react";
import Dashboard from "./pages/Dashboard";
import Routing from "./pages/Routing";
import Pipeline from "./pages/Pipeline";
import Tasks from "./pages/Tasks";
import Memory from "./pages/Memory";
import Permissions from "./pages/Permissions";
import Mcp from "./pages/Mcp";
import Agent from "./pages/Agent";
import Scripts from "./pages/Scripts";
import Sessions from "./pages/Sessions";
import Audit from "./pages/Audit";
import Guard from "./pages/Guard";

type Page =
  | "dashboard"
  | "routing"
  | "pipeline"
  | "tasks"
  | "memory"
  | "permissions"
  | "mcp"
  | "agent"
  | "scripts"
  | "sessions"
  | "audit"
  | "guard";

export default function App() {
  const [page, setPage] = useState<Page>("dashboard");

  const nav: { key: Page; label: string }[] = [
    { key: "dashboard", label: "Dashboard" },
    { key: "routing", label: "Modell-Routing" },
    { key: "pipeline", label: "REA/ECC Pipeline" },
    { key: "tasks", label: "Express-Tasks" },
    { key: "memory", label: "Memory" },
    { key: "permissions", label: "Permissions" },
    { key: "mcp", label: "MCP-Tools" },
    { key: "agent", label: "Agent" },
    { key: "scripts", label: "Skripte" },
    { key: "sessions", label: "Sessions" },
    { key: "audit", label: "Audit" },
    { key: "guard", label: "Guard" },
  ];

  return (
    <div style={{ fontFamily: "system-ui, sans-serif", margin: 0, padding: "1rem 2rem" }}>
      <header style={{ display: "flex", alignItems: "center", gap: "2rem", borderBottom: "1px solid #333" }}>
        <h1 style={{ fontSize: "1.2rem" }}>B3nBox Studio</h1>
        <nav style={{ display: "flex", gap: "1rem" }}>
          {nav.map((n) => (
            <button
              key={n.key}
              onClick={() => setPage(n.key)}
              style={{
                padding: "0.4rem 0.8rem",
                cursor: "pointer",
                background: page === n.key ? "#4f46e5" : "transparent",
                color: page === n.key ? "#fff" : "#4f46e5",
                border: "1px solid #4f46e5",
                borderRadius: 6,
              }}
            >
              {n.label}
            </button>
          ))}
        </nav>
      </header>
      <main style={{ paddingTop: "1rem" }}>
        {page === "dashboard" && <Dashboard />}
        {page === "routing" && <Routing />}
        {page === "pipeline" && <Pipeline />}
        {page === "tasks" && <Tasks />}
        {page === "memory" && <Memory />}
        {page === "permissions" && <Permissions />}
        {page === "mcp" && <Mcp />}
        {page === "agent" && <Agent />}
        {page === "scripts" && <Scripts />}
        {page === "sessions" && <Sessions />}
        {page === "audit" && <Audit />}
        {page === "guard" && <Guard />}
      </main>
    </div>
  );
}
