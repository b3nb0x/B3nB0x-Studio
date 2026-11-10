import { useEffect, useState } from "react";
import { api } from "../api";

export default function Guard() {
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  const load = () =>
    api.guardBlocked().then((r) => {
      if (r.error) setError(r.error);
      else setData(r);
    });

  useEffect(() => {
    load();
    const timer = setInterval(load, 5000);
    return () => clearInterval(timer);
  }, []);

  const config = data?.config;
  const blocked = data?.blocked ?? [];

  return (
    <section>
      <h2>Injection-Guard</h2>
      <p>
        Prompt-Injection-Schutz: Scanner-Signaturen, Tool-Output-Isolation, Argument-Validierung und optionaler
        LLM-Klassifikator. Aktualisiert alle 5s.
      </p>

      {error && <p style={{ color: "crimson" }}>{error}</p>}

      {config && (
        <div
          style={{
            background: "#422006",
            border: "1px solid #f59e0b",
            borderRadius: 6,
            padding: 12,
            marginBottom: "1rem",
          }}
        >
          <strong>Guard-Status:</strong>{" "}
          {config.enabled ? (
            <span style={{ color: "green" }}>aktiv</span>
          ) : (
            <span style={{ color: "crimson" }}>deaktiviert</span>
          )}{" "}
          — Modus: <strong>{config.mode}</strong> — LLM-Klassifikator:{" "}
          {config.llm_check ? (
            <span style={{ color: "green" }}>an (Zweitmeinung bei Verdacht)</span>
          ) : (
            <span style={{ color: "#94a3b8" }}>aus</span>
          )}
        </div>
      )}

      <h3>Block-Log ({blocked.length})</h3>
      <table cellPadding={6} style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th align="left">Zeit</th>
            <th align="left">Kontext</th>
            <th align="left">Grund</th>
            <th align="left">Auszug</th>
          </tr>
        </thead>
        <tbody>
          {blocked.map((b: any) => (
            <tr key={b.id}>
              <td>{b.created_at}</td>
              <td>
                <code>{b.context}</code>
              </td>
              <td>{b.reason}</td>
              <td>
                <span style={{ fontFamily: "monospace", fontSize: 12, color: "#94a3b8" }}>
                  {b.excerpt.length > 60 ? `${b.excerpt.slice(0, 60)}…` : b.excerpt}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {blocked.length === 0 && (
        <p style={{ color: "#94a3b8" }}>Keine geblockten Versuche — entweder ruhiges System oder Guard im log-only-Modus.</p>
      )}
    </section>
  );
}
