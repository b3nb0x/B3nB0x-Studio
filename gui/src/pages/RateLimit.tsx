import { useEffect, useState } from "react";
import { api } from "../api";

export default function RateLimit() {
  const [status, setStatus] = useState<{
    enabled: boolean;
    requests_per_minute: number;
    llm_requests_per_minute: number;
  } | null>(null);
  const [rpm, setRpm] = useState("150");
  const [llmRpm, setLlmRpm] = useState("30");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = () =>
    api.rateLimit().then((r) => {
      setStatus(r);
      setRpm(String(r.requests_per_minute));
      setLlmRpm(String(r.llm_requests_per_minute));
    });

  useEffect(() => {
    load();
    const timer = setInterval(load, 10000);
    return () => clearInterval(timer);
  }, []);

  const save = async () => {
    setError(null);
    setMessage(null);
    try {
      const r = await api.updateRateLimit(Number(rpm), Number(llmRpm));
      setStatus(r);
      setMessage(
        `Gespeichert: ${r.requests_per_minute} Requests/min, ${r.llm_requests_per_minute} LLM-Requests/min`
      );
    } catch (e) {
      setError(String(e).includes("403") ? "Admin-Rolle erforderlich" : String(e));
    }
  };

  return (
    <section>
      <h2>Rate-Limiting</h2>
      <p>
        Token-Bucket pro Client. LLM-Endpunkte (Chat, Pipeline, Agent, Sessions) haben ein eigenes,
        niedrigeres Limit. Änderungen greifen sofort (nur Admin).
      </p>

      {status && (
        <div
          style={{
            background: status.enabled ? "#052e16" : "#422006",
            border: `1px solid ${status.enabled ? "#16a34a" : "#f59e0b"}`,
            borderRadius: 6,
            padding: 12,
            marginBottom: "1rem",
          }}
        >
          <strong>Guard-Status:</strong>{" "}
          {status.enabled ? (
            <span style={{ color: "green" }}>aktiv</span>
          ) : (
            <span style={{ color: "#fbbf24" }}>deaktiviert</span>
          )}{" "}
          — {status.requests_per_minute} Requests/min · {status.llm_requests_per_minute} LLM-Requests/min
        </div>
      )}

      <h3>Limits anpassen</h3>
      <div style={{ display: "flex", gap: "1rem", alignItems: "center" }}>
        <label>
          Requests/min:{" "}
          <input
            type="number"
            min={1}
            value={rpm}
            onChange={(e) => setRpm(e.target.value)}
            style={{ width: 100 }}
          />
        </label>
        <label>
          LLM-Requests/min:{" "}
          <input
            type="number"
            min={1}
            value={llmRpm}
            onChange={(e) => setLlmRpm(e.target.value)}
            style={{ width: 100 }}
          />
        </label>
        <button onClick={save}>Speichern</button>
      </div>
      {message && <p style={{ color: "green" }}>{message}</p>}
      {error && <p style={{ color: "crimson" }}>{error}</p>}
    </section>
  );
}
