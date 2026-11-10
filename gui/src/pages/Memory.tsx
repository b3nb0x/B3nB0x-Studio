import { useEffect, useState } from "react";
import { api } from "../api";

export default function Memory() {
  const [recent, setRecent] = useState<any[]>([]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<any[] | null>(null);
  const [kind, setKind] = useState("project");
  const [key, setKey] = useState("");
  const [content, setContent] = useState("");

  const load = () => api.recentMemory(15).then((r) => setRecent(r.results ?? []));
  useEffect(() => {
    load();
  }, []);

  const search = async () => {
    if (!query) return;
    const r = await api.searchMemory(query);
    setResults(r.results ?? []);
  };

  const add = async () => {
    if (!key || !content) return;
    await api.addMemory(kind, key, content);
    setKey("");
    setContent("");
    load();
  };

  return (
    <section>
      <h2>Memory-Layer</h2>
      <p>Projektkontext und Task-Historie — die Pipeline injiziert relevante Einträge automatisch in die Analyse-Phase.</p>

      <h3>Suchen</h3>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <input style={{ flex: 1 }} placeholder="Suchbegriff" value={query} onChange={(e) => setQuery(e.target.value)} />
        <button onClick={search}>Suchen</button>
      </div>
      {results && (
        <ul>
          {results.length === 0 && <li>Keine Treffer</li>}
          {results.map((m) => (
            <li key={m.id}>
              <strong>[{m.kind}]</strong> {m.key}: {m.content.slice(0, 100)}
            </li>
          ))}
        </ul>
      )}

      <h3>Eintrag hinzufügen</h3>
      <div style={{ display: "flex", gap: "0.5rem" }}>
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="project">project</option>
          <option value="task">task</option>
          <option value="note">note</option>
        </select>
        <input placeholder="key" value={key} onChange={(e) => setKey(e.target.value)} />
        <input style={{ flex: 1 }} placeholder="inhalt" value={content} onChange={(e) => setContent(e.target.value)} />
        <button onClick={add}>Hinzufügen</button>
      </div>

      <h3>Letzte Einträge</h3>
      <ul>
        {recent.map((m) => (
          <li key={m.id}>
            <strong>[{m.kind}]</strong> {m.key} <em>({m.created_at})</em>
          </li>
        ))}
      </ul>
    </section>
  );
}
