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
import Login from "./pages/Login";
import RateLimit from "./pages/RateLimit";

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
  | "guard"
  | "login"
  | "ratelimit";

const nav: { key: Page; label: string; hint: string }[] = [
  { key: "dashboard", label: "Dashboard", hint: "Übersicht: Backends, Modelle, Status" },
  { key: "routing", label: "Modell-Routing", hint: "Route-Key → Backend + Modell, live umschaltbar" },
  { key: "pipeline", label: "REA/ECC Pipeline", hint: "Analyse → Plan → Implementieren → Kontrolle, pro Phase eigenes Modell" },
  { key: "tasks", label: "Express-Tasks", hint: "Einmal-Aufgaben an ein Modell" },
  { key: "memory", label: "Memory", hint: "Projektwissen dauerhaft speichern und durchsuchen" },
  { key: "permissions", label: "Permissions", hint: "Erlauben/Verbieten pro Aktion" },
  { key: "mcp", label: "MCP-Tools", hint: "Externe MCP-Server verbinden und Tools aufrufen" },
  { key: "agent", label: "Agent", hint: "Autonomer Loop mit Tool-Nutzung" },
  { key: "scripts", label: "Skripte", hint: "Eigene Skripte registrieren und ausführen" },
  { key: "sessions", label: "Sessions", hint: "Gespräche mit Verlauf" },
  { key: "audit", label: "Audit", hint: "Protokoll aller LLM-Interaktionen" },
  { key: "guard", label: "Guard", hint: "Injection-Schutz-Protokoll" },
  { key: "login", label: "Login", hint: "Mit API-Key anmelden" },
  { key: "ratelimit", label: "Rate-Limit", hint: "Limits ansehen und anpassen (Admin)" },
];

const HINTS: Partial<Record<Page, string>> = {
  dashboard: "Startpunkt: Backends, Modelle und Systemstatus auf einen Blick.",
  routing: "Jeder Route-Key zeigt auf ein Backend + Modell. Umschalten wirkt sofort, ohne Neustart.",
  pipeline: "Vier Phasen, vier Modelle: Analyse, Plan, Implementieren, Kontrolle — jede Phase live umschaltbar.",
  mcp: "Server aus der Grundauswahl per Klick hinzufügen — keine TOML-Datei nötig.",
  sessions: "Gespräche bleiben mit Verlauf gespeichert. Token nach dem Login wird lokal gespeichert.",
  login: "Dein API-Key erzeugt eine Session. Der Key selbst wird nie gespeichert — nur ein Hash.",
  ratelimit: "Admin darf Limits zur Laufzeit ändern; LLM-Endpunkte haben ein eigenes, niedrigeres Limit.",
};

export default function App() {
  const [page, setPage] = useState<Page>("dashboard");

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-glow">B3nB0x</span> Studio
          <div className="brand-sub">LLM Manager</div>
        </div>
        <nav className="sidebar-nav">
          {nav.map((n) => (
            <button
              key={n.key}
              onClick={() => setPage(n.key)}
              title={n.hint}
              className={page === n.key ? "nav-item active" : "nav-item"}
            >
              {n.label}
            </button>
          ))}
        </nav>
      </aside>
      <main className="content">
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
        {page === "login" && <Login />}
        {page === "ratelimit" && <RateLimit />}
      </main>
      <aside className="hint-panel">
        <h3>Kurzhilfe</h3>
        <p>{HINTS[page] ?? "Wähle links einen Bereich."}</p>
        <p className="hint-foot">
          Erst einloggen (Login), dann loslegen. Bei Fragen hilft der
          Dashboard-Status: Zeigt, ob Backends und Modelle erreichbar sind.
        </p>
      </aside>
    </div>
  );
}
