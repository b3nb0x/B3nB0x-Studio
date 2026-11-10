import { useEffect, useState } from "react";
import { api } from "../api";

export default function Dashboard() {
  const [health, setHealth] = useState<any>(null);
  const [models, setModels] = useState<any[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.health().then(setHealth).catch((e) => setError(String(e)));
    api.models().then((r) => setModels(r.models ?? [])).catch(() => {});
  }, []);

  return (
    <section>
      <h2>Dashboard</h2>
      {error && <p style={{ color: "crimson" }}>LLM Manager nicht erreichbar: {error}</p>}
      {health && (
        <p>
          Status: <strong>{health.status}</strong> — Service: {health.service}
        </p>
      )}
      <h3>Verfügbare Modelle ({models.length})</h3>
      <ul>
        {models.map((m, i) => (
          <li key={i}>
            <code>{m.id}</code> @ {m.backend}
          </li>
        ))}
      </ul>
      {models.length === 0 && <p>Keine Modelle gefunden — Backends in config/b3nbox.toml konfigurieren.</p>}
    </section>
  );
}
