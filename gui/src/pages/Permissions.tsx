import { useEffect, useState } from "react";
import { api } from "../api";

export default function Permissions() {
  const [mode, setMode] = useState("");
  const [rules, setRules] = useState<any[]>([]);
  const [effect, setEffect] = useState("deny");
  const [action, setAction] = useState("script");
  const [scope, setScope] = useState("");
  const [checkResult, setCheckResult] = useState<string | null>(null);

  const load = () =>
    api.permissions().then((r) => {
      setMode(r.mode);
      setRules(r.rules ?? []);
    });
  useEffect(() => {
    load();
  }, []);

  const add = async () => {
    await api.addPermission({ effect, action, scope: scope || undefined });
    setScope("");
    load();
  };

  const check = async () =>
    api.checkPermission(action, scope || undefined).then((r) => setCheckResult(JSON.stringify(r, null, 2)));

  return (
    <section>
      <h2>Permission-System</h2>
      <p>
        Modus: <strong>{mode}</strong> — letzte passende Regel gewinnt. Aktionen: <code>chat</code>, <code>pipeline</code>,{" "}
        <code>task</code>, <code>script</code>, <code>mcp_tool</code>. Wildcard: <code>*</code>.
      </p>

      <h3>Regeln ({rules.length})</h3>
      <table cellPadding={6} style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th align="left">Effekt</th>
            <th align="left">Aktion</th>
            <th align="left">Scope</th>
          </tr>
        </thead>
        <tbody>
          {rules.map((r, i) => (
            <tr key={i}>
              <td style={{ color: r.effect === "deny" ? "crimson" : "green" }}>{r.effect}</td>
              <td>
                <code>{r.action}</code>
              </td>
              <td>{r.scope ?? "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>Regel hinzufügen / prüfen</h3>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <select value={effect} onChange={(e) => setEffect(e.target.value)}>
          <option value="allow">allow</option>
          <option value="deny">deny</option>
        </select>
        <input placeholder="action" value={action} onChange={(e) => setAction(e.target.value)} />
        <input placeholder="scope (optional)" value={scope} onChange={(e) => setScope(e.target.value)} />
        <button onClick={add}>Regel speichern</button>
        <button onClick={check}>Aktion prüfen</button>
      </div>
      {checkResult && (
        <pre style={{ background: "#f5f5f5", padding: 8 }}>{checkResult}</pre>
      )}
    </section>
  );
}
