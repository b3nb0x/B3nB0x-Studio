import { useEffect, useState } from "react";
import { api } from "../api";

export default function Mcp() {
  const [servers, setServers] = useState<[string, any[]][]>([]);
  const [result, setResult] = useState<string | null>(null);
  const [server, setServer] = useState("");
  const [tool, setTool] = useState("");
  const [args, setArgs] = useState("{}");

  const load = () =>
    api.mcpTools().then((r) => {
      const s = r.servers ?? [];
      setServers(s);
      if (s.length > 0) {
        setServer(s[0][0]);
        setTool(s[0][1][0]?.name ?? "");
      }
    });
  useEffect(() => {
    load();
  }, []);

  const call = async () => {
    try {
      const r = await api.mcpCall(server, tool, JSON.parse(args));
      setResult(JSON.stringify(r, null, 2));
    } catch (e) {
      setResult(`Fehler: ${e}`);
    }
  };

  return (
    <section>
      <h2>MCP-Tools</h2>
      <p>Tools externer MCP-Server (Model Context Protocol, JSON-RPC über stdio).</p>
      {servers.length === 0 && <p>Keine MCP-Server verbunden — in config/b3nbox.toml unter [mcp.*] konfigurieren.</p>}
      {servers.map(([name, tools]) => (
        <div key={name}>
          <h3>{name}</h3>
          <ul>
            {tools.map((t) => (
              <li key={t.name}>
                <code>{t.name}</code> — {t.description}
              </li>
            ))}
          </ul>
        </div>
      ))}

      {servers.length > 0 && (
        <>
          <h3>Tool aufrufen</h3>
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <select value={server} onChange={(e) => setServer(e.target.value)}>
              {servers.map(([name]) => (
                <option key={name}>{name}</option>
              ))}
            </select>
            <select value={tool} onChange={(e) => setTool(e.target.value)}>
              {(servers.find(([n]) => n === server)?.[1] ?? []).map((t) => (
                <option key={t.name}>{t.name}</option>
              ))}
            </select>
            <input style={{ flex: 1 }} value={args} onChange={(e) => setArgs(e.target.value)} />
            <button onClick={call}>Aufrufen</button>
          </div>
          {result && <pre style={{ background: "#f5f5f5", padding: 8 }}>{result}</pre>}
        </>
      )}
    </section>
  );
}
