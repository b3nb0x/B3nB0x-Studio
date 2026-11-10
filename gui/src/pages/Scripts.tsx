import { useEffect, useState } from "react";
import { api } from "../api";

export default function Scripts() {
  const [scripts, setScripts] = useState<any[]>([]);
  const [schedule, setSchedule] = useState<any[]>([]);
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");
  const [interval, setInterval] = useState(60);
  const [result, setResult] = useState<string | null>(null);

  const load = () => {
    api.listScripts().then((r) => setScripts(r.scripts ?? []));
    api.listSchedule().then((r) => setSchedule(r.schedule ?? []));
  };
  useEffect(() => {
    load();
  }, []);

  const register = async () => {
    if (!name || !command) return;
    const r = await api.registerScript({
      name,
      command,
      args: args ? args.split(" ") : [],
      description: "",
    });
    setResult(JSON.stringify(r));
    setName("");
    setCommand("");
    setArgs("");
    load();
  };

  const run = async (scriptName: string, approval: boolean) => {
    const r = await api.runScript(scriptName, approval);
    setResult(JSON.stringify(r, null, 2));
  };

  const addSchedule = async (scriptName: string) => {
    const r = await api.addSchedule({
      name: `schedule-${scriptName}-${Date.now() % 10000}`,
      script: scriptName,
      args: [],
      every_seconds: interval,
      approval_required: true,
    });
    setResult(JSON.stringify(r));
    load();
  };

  const removeSchedule = async (scheduleName: string) => {
    await api.removeSchedule(scheduleName);
    load();
  };

  return (
    <section>
      <h2>Skripte &amp; Scheduler</h2>

      <h3>Skript registrieren</h3>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <input placeholder="name" value={name} onChange={(e) => setName(e.target.value)} />
        <input placeholder="command (z.B. scripts/backup.sh)" value={command} onChange={(e) => setCommand(e.target.value)} />
        <input placeholder="args (leerzeichen-getrennt)" value={args} onChange={(e) => setArgs(e.target.value)} />
        <button onClick={register}>Registrieren</button>
      </div>

      <h3>Registrierte Skripte ({scripts.length})</h3>
      <table cellPadding={6} style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th align="left">Name</th>
            <th align="left">Command</th>
            <th align="left">Aktionen</th>
          </tr>
        </thead>
        <tbody>
          {scripts.map((s) => (
            <tr key={s.name}>
              <td>
                <code>{s.name}</code>
              </td>
              <td>
                {s.command} {s.args?.join(" ")}
              </td>
              <td>
                <button onClick={() => run(s.name, false)}>Ausführen</button>{" "}
                <button onClick={() => run(s.name, true)}>Mit Freigabe</button>{" "}
                <label>
                  alle {interval}s planen:{" "}
                  <input
                    type="number"
                    min={5}
                    value={interval}
                    onChange={(e) => setInterval(Number(e.target.value))}
                    style={{ width: 70 }}
                  />
                </label>{" "}
                <button onClick={() => addSchedule(s.name)}>Schedule</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>Schedules ({schedule.length})</h3>
      <ul>
        {schedule.map((s) => (
          <li key={s.name}>
            <strong>{s.script}</strong> alle {s.every_seconds}s
            {s.approval_required && <em> (Freigabe erforderlich)</em>}
            {s.next_run_in_seconds != null && <> — nächster Lauf in {s.next_run_in_seconds}s</>}{" "}
            <button onClick={() => removeSchedule(s.name)}>Entfernen</button>
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
