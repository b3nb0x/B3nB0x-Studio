import { useEffect, useState } from "react";
import { api } from "../api";

type CatalogEntry = { name: string; description: string; command: string; args: string[] };
type ServerEntry = { name: string; command: string; args: string[] };

export default function Mcp() {
  const [servers, setServers] = useState<[string, any[]][]>([]);
  const [configured, setConfigured] = useState<ServerEntry[]>([]);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [result, setResult] = useState<string | null>(null);
  const [server, setServer] = useState("");
  const [tool, setTool] = useState("");
  const [args, setArgs] = useState("{}");
  const [newName, setNewName] = useState("");
  const [newCommand, setNewCommand] = useState("");
  const [newArgs, setNewArgs] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  const load = () =>
    Promise.all([api.mcpTools(), api.mcpServers().catch(() => ({ servers: [] })), api.mcpCatalog().catch(() => ({ catalog: [] }))])
      .then(([tools, cfg, cat]) => {
        const s = tools.servers ?? [];
        setServers(s);
        setConfigured(cfg.servers ?? []);
        setCatalog(cat.catalog ?? []);
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

  const addServer = async (name: string, command: string, argsList: string[]) => {
    setMsg(null);
    try {
      const r = await api.mcpAddServer(name, command, argsList);
      if (r.error) setMsg(`Fehler: ${r.error}`);
      else setMsg(`MCP-Server "${name}" gestartet (${(r.tools ?? []).length} Tools)`);
    } catch (e) {
      setMsg(`Fehler: ${e}`);
    }
    load();
  };

  const addCustom = () => {
    const argsList = newArgs.trim() ? newArgs.trim().split(/\s+/) : [];
    addServer(newName.trim(), newCommand.trim(), argsList);
    setNewName("");
    setNewCommand("");
    setNewArgs("");
  };

  const removeServer = async (name: string) => {
    setMsg(null);
    try {
      await api.mcpRemoveServer(name);
      setMsg(`MCP-Server "${name}" entfernt`);
    } catch (e) {
      setMsg(`Fehler: ${e}`);
    }
    load();
  };

  const activeNames = new Set(configured.map((s) => s.name));

  return (
    <section>
      <h2>MCP-Server &amp; Tools</h2>
      <p>Externe MCP-Server (Model Context Protocol, JSON-RPC über stdio) — hier verwalten, keine TOML-Datei nötig.</p>
      {msg && <p><strong>{msg}</strong></p>}

      <h3>Grundauswahl (Schnellstart)</h3>
      <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem" }}>
        {catalog.map((c) => (
          <button key={c.name} disabled={activeNames.has(c.name)} title={c.description} onClick={() => addServer(c.name, c.command, c.args)}>
            {activeNames.has(c.name) ? `✓ ${c.name}` : `+ ${c.name}`}
          </button>
        ))}
      </div>

      <h3>Eigener Server hinzufügen</h3>
      <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
        <input placeholder="Name (z.B. my-server)" value={newName} onChange={(e) => setNewName(e.target.value)} />
        <input placeholder="Befehl (z.B. npx)" value={newCommand} onChange={(e) => setNewCommand(e.target.value)} />
        <input placeholder="Argumente (durch Leerzeichen getrennt)" value={newArgs} onChange={(e) => setNewArgs(e.target.value)} />
        <button onClick={addCustom}>Hinzufügen &amp; starten</button>
      </div>

      <h3>Konfigurierte Server</h3>
      {configured.length === 0 && <p>Keine MCP-Server konfiguriert — oben aus der Grundauswahl wählen.</p>}
      {configured.map((s) => (
        <div key={s.name} style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          <code>{s.name}</code>
          <span style={{ opacity: 0.7 }}>{s.command} {s.args.join(" ")}</span>
          <button onClick={() => removeServer(s.name)}>Entfernen</button>
        </div>
      ))}

      {servers.length > 0 && (
        <>
          <h3>Verfügbare Tools</h3>
          {servers.map(([name, tools]) => (
            <div key={name}>
              <h4>{name}</h4>
              <ul>
                {tools.map((t: any) => (
                  <li key={t.name}>
                    <code>{t.name}</code> — {t.description}
                  </li>
                ))}
              </ul>
            </div>
          ))}

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
          {result && <pre style={{ background: "#0f172a", padding: 8 }}>{result}</pre>}
        </>
      )}
    </section>
  );
}
