import { useEffect, useState } from "react";
import { api } from "../api";

export default function Audit() {
  const [entries, setEntries] = useState<any[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = () =>
    api.recentAudit(30).then((r) => {
      if (r.error) setError(r.error);
      else setEntries(r.entries ?? []);
    });

  useEffect(() => {
    load();
    const timer = setInterval(load, 5000);
    return () => clearInterval(timer);
  }, []);

  return (
    <section>
      <h2>Audit-Log</h2>
      <p>Jede LLM-Interaktion: Route, Backend, Modell, Dauer, Status, Prompt-Hash (keine Inhalte). Aktualisiert alle 5s.</p>
      {error && <p style={{ color: "crimson" }}>{error}</p>}
      <table cellPadding={6} style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th align="left">Zeit</th>
            <th align="left">Route</th>
            <th align="left">Backend</th>
            <th align="left">Modell</th>
            <th align="right">Dauer</th>
            <th align="left">Status</th>
            <th align="right">Prompt</th>
            <th align="right">Antwort</th>
            <th align="left">Hash</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((e) => (
            <tr key={e.id}>
              <td>{e.created_at}</td>
              <td>
                <code>{e.route}</code>
              </td>
              <td>{e.backend}</td>
              <td>
                <code>{e.model}</code>
              </td>
              <td align="right">{e.duration_ms} ms</td>
              <td style={{ color: e.status === "ok" ? "green" : "crimson" }}>{e.status}</td>
              <td align="right">{e.prompt_chars}</td>
              <td align="right">{e.response_chars}</td>
              <td>
                <code>{e.prompt_hash.slice(0, 8)}</code>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {entries.length === 0 && !error && <p>Keine Einträge.</p>}
    </section>
  );
}
