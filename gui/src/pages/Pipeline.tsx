import { useEffect, useState } from "react";
import { api } from "../api";

const PHASES = ["analyse", "plan", "implement", "control"];

export default function Pipeline() {
  const [task, setTask] = useState("");
  const [run, setRun] = useState<any>(null);
  const [runs, setRuns] = useState<any[]>([]);
  const [busy, setBusy] = useState(false);
  const [phase, setPhase] = useState("analyse");
  const [backend, setBackend] = useState("");
  const [model, setModel] = useState("");

  const loadRuns = () => api.listRuns().then((r) => setRuns(r.runs ?? []));
  useEffect(() => {
    loadRuns();
  }, []);

  const start = async () => {
    if (!task) return;
    setBusy(true);
    try {
      const result = await api.startRun(task);
      setRun(result);
    } finally {
      setBusy(false);
      loadRuns();
    }
  };

  const switchModel = async () => {
    if (!backend || !model) return;
    await api.switchPhase(phase, backend, model);
  };

  return (
    <section>
      <h2>REA/ECC Pipeline</h2>
      <p>
        Softwareentwicklung in Phasen: <strong>Analyse → Plan → Implementieren → Kontrolle</strong>. Jede Phase kann
        live auf ein anderes lokales Modell geschaltet werden.
      </p>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <input
          style={{ flex: 1 }}
          placeholder="Aufgabe beschreiben (z.B. 'CLI-Tool für Log-Analyse in Rust')"
          value={task}
          onChange={(e) => setTask(e.target.value)}
        />
        <button onClick={start} disabled={busy}>
          {busy ? "Läuft..." : "Pipeline starten"}
        </button>
      </div>

      <h3>Phase live umschalten</h3>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <select value={phase} onChange={(e) => setPhase(e.target.value)}>
          {PHASES.map((p) => (
            <option key={p}>{p}</option>
          ))}
        </select>
        <input placeholder="backend" value={backend} onChange={(e) => setBackend(e.target.value)} />
        <input placeholder="modell" value={model} onChange={(e) => setModel(e.target.value)} />
        <button onClick={switchModel}>Umschalten</button>
      </div>

      {run && (
        <div>
          <h3>Letzter Run: {run.id.slice(0, 8)} ({run.status})</h3>
          {run.error && <p style={{ color: "crimson" }}>{run.error}</p>}
          {run.results?.map((r: any, i: number) => (
            <details key={i} style={{ marginBottom: 8 }}>
              <summary>
                <strong>{r.phase}</strong> — {r.backend} / {r.model} ({r.duration_ms} ms)
              </summary>
              <pre style={{ whiteSpace: "pre-wrap", background: "#0f172a", padding: 8 }}>{r.output}</pre>
            </details>
          ))}
        </div>
      )}

      <h3>Runs ({runs.length})</h3>
      <ul>
        {runs.map((r) => (
          <li key={r.id}>
            <code>{r.id.slice(0, 8)}</code> — {r.status} — {r.task.slice(0, 60)}
          </li>
        ))}
      </ul>
    </section>
  );
}
