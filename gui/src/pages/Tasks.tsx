import { useEffect, useState } from "react";
import { api } from "../api";

const PRESETS = [
  { name: "Reboot-Scheduler", prompt: "Bestätige den geplanten Reboot (Wartungsfenster prüfen).", route: "scheduler" },
  { name: "System-Check", prompt: "Prüfe Systemstatus: Speicher, Disk, kritische Services.", route: "default" },
  { name: "Log-Analyse", prompt: "Analysiere die letzten Fehler-Logs und fasse zusammen.", route: "analyse" },
];

export default function Tasks() {
  const [tasks, setTasks] = useState<any[]>([]);
  const [result, setResult] = useState<string | null>(null);

  const load = () => api.listTasks().then((r) => setTasks(r.tasks ?? []));
  useEffect(() => {
    load();
  }, []);

  const addPreset = async (p: { name: string; prompt: string; route: string }) => {
    await api.createTask({ name: p.name, prompt: p.prompt, route: p.route });
    load();
  };

  const run = async (id: string) => {
    const r = await api.runTask(id);
    setResult(JSON.stringify(r, null, 2));
  };

  return (
    <section>
      <h2>Express-Tasks</h2>
      <h3>Standard-Presets</h3>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        {PRESETS.map((p) => (
          <button key={p.name} onClick={() => addPreset(p)}>
            + {p.name}
          </button>
        ))}
      </div>
      <h3>Registrierte Tasks ({tasks.length})</h3>
      <ul>
        {tasks.map((t, i) => (
          <li key={i}>
            <strong>{t.name}</strong> (Route: {t.route}){" "}
            <button onClick={() => run(t.id)}>Ausführen</button>
          </li>
        ))}
      </ul>
      {result && (
        <details>
          <summary>Letztes Ergebnis</summary>
          <pre style={{ whiteSpace: "pre-wrap", background: "#f5f5f5", padding: 8 }}>{result}</pre>
        </details>
      )}
    </section>
  );
}
