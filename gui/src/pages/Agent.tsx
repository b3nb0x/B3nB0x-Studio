import { useState } from "react";
import { api } from "../api";

export default function Agent() {
  const [task, setTask] = useState("");
  const [route, setRoute] = useState("agent");
  const [maxIterations, setMaxIterations] = useState(8);
  const [busy, setBusy] = useState(false);
  const [run, setRun] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const start = async () => {
    if (!task) return;
    setBusy(true);
    setError(null);
    setRun(null);
    try {
      const result = await api.agentRun(task, route, maxIterations);
      if (result.error) setError(result.error);
      else setRun(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section>
      <h2>Agent</h2>
      <p>
        Der Agent löst Aufgaben selbstständig mit Tools (MCP) — Schritt für Schritt, mit Zwischenschritten und Tool-Aufrufen.
      </p>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <input
          style={{ flex: 1 }}
          placeholder="Aufgabe (z.B. 'Lies die Projektdateien und fasse die Struktur zusammen')"
          value={task}
          onChange={(e) => setTask(e.target.value)}
        />
        <input placeholder="route" value={route} onChange={(e) => setRoute(e.target.value)} style={{ width: 100 }} />
        <input
          type="number"
          min={1}
          max={32}
          value={maxIterations}
          onChange={(e) => setMaxIterations(Number(e.target.value))}
          style={{ width: 70 }}
          title="Max. Iterationen"
        />
        <button onClick={start} disabled={busy}>
          {busy ? "Agent läuft..." : "Starten"}
        </button>
      </div>

      {error && <p style={{ color: "crimson" }}>{error}</p>}

      {run && (
        <div>
          <h3>
            Status: <strong style={{ color: run.status === "completed" ? "green" : "orange" }}>{run.status}</strong>{" "}
            ({run.iterations_used} Iterationen)
          </h3>
          {run.answer && (
            <div style={{ background: "#f0fdf4", border: "1px solid #bbf7d0", padding: 12, borderRadius: 6 }}>
              <strong>Antwort:</strong>
              <p style={{ whiteSpace: "pre-wrap", margin: "8px 0 0" }}>{run.answer}</p>
            </div>
          )}
          <h4>Schritte</h4>
          <ol>
            {run.steps.map((s: any, i: number) => (
              <li key={i}>
                {s.tool ? (
                  <>
                    <code>{s.tool}</code> → <span style={{ color: "#666" }}>{s.tool_result?.slice(0, 120)}</span>
                  </>
                ) : (
                  s.thought
                )}
              </li>
            ))}
          </ol>
        </div>
      )}
    </section>
  );
}
