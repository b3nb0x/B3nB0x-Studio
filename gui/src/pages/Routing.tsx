import { useEffect, useState } from "react";
import { api } from "../api";

export default function Routing() {
  const [routing, setRouting] = useState<Record<string, { backend: string; model: string }>>({});
  const [key, setKey] = useState("");
  const [backend, setBackend] = useState("");
  const [model, setModel] = useState("");

  const load = () => api.routing().then((r) => setRouting(r.routing ?? {}));
  useEffect(() => {
    load();
  }, []);

  const save = async () => {
    if (!key || !backend || !model) return;
    await api.setRoute(key, backend, model);
    setKey("");
    setBackend("");
    setModel("");
    load();
  };

  return (
    <section>
      <h2>Modell-Routing</h2>
      <p>Jeder Route (Task-Typ oder Pipeline-Phase) kann ein Backend + Modell zugewiesen werden.</p>
      <table cellPadding={6} style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th align="left">Route</th>
            <th align="left">Backend</th>
            <th align="left">Modell</th>
          </tr>
        </thead>
        <tbody>
          {Object.entries(routing).map(([k, v]) => (
            <tr key={k}>
              <td>
                <code>{k}</code>
              </td>
              <td>{v.backend}</td>
              <td>
                <code>{v.model}</code>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <h3>Route setzen / ändern (Live-Switch)</h3>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <input placeholder="route (z.B. analyse)" value={key} onChange={(e) => setKey(e.target.value)} />
        <input placeholder="backend (z.B. ollama)" value={backend} onChange={(e) => setBackend(e.target.value)} />
        <input placeholder="modell" value={model} onChange={(e) => setModel(e.target.value)} />
        <button onClick={save}>Speichern</button>
      </div>
    </section>
  );
}
