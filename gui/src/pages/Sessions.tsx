import { useEffect, useState } from "react";
import { api } from "../api";

export default function Sessions() {
  const [sessions, setSessions] = useState<any[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [messages, setMessages] = useState<any[]>([]);
  const [message, setMessage] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);

  const load = () => api.listSessions().then((r) => setSessions(r.sessions ?? []));
  useEffect(() => {
    load();
  }, []);

  const openSession = async (id: string) => {
    setActiveId(id);
    const r = await api.getSession(id);
    setMessages(r.messages ?? []);
  };

  const create = async () => {
    if (!name) return;
    const s = await api.createSession(name);
    setName("");
    load();
    openSession(s.id);
  };

  const send = async () => {
    if (!message || !activeId) return;
    setBusy(true);
    try {
      const r = await api.sessionChat(activeId, message);
      if (r.error) {
        setMessages((prev) => [...prev, { role: "error", content: r.error }]);
      } else {
        setMessages((prev) => [
          ...prev,
          { role: "user", content: message },
          { role: "assistant", content: r.content },
        ]);
      }
      setMessage("");
      load();
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string) => {
    await api.deleteSession(id);
    if (activeId === id) {
      setActiveId(null);
      setMessages([]);
    }
    load();
  };

  return (
    <section>
      <h2>Sessions</h2>
      <p>Persistente Konversationen — die volle Historie wird als Kontext mitgesendet.</p>

      <div style={{ display: "flex", gap: "0.5rem", marginBottom: "1rem" }}>
        <input placeholder="Session-Name" value={name} onChange={(e) => setName(e.target.value)} />
        <button onClick={create}>Neue Session</button>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "240px 1fr", gap: "1rem" }}>
        <div>
          <h4>Sessions ({sessions.length})</h4>
          <ul style={{ listStyle: "none", padding: 0 }}>
            {sessions.map((s) => (
              <li key={s.id} style={{ marginBottom: 6 }}>
                <button
                  onClick={() => openSession(s.id)}
                  style={{
                    cursor: "pointer",
                    background: activeId === s.id ? "#4f46e5" : "#f5f5f5",
                    color: activeId === s.id ? "#fff" : "#000",
                    border: "1px solid #ccc",
                    borderRadius: 6,
                    padding: "0.4rem 0.6rem",
                    width: "100%",
                    textAlign: "left",
                  }}
                >
                  {s.name} ({s.message_count})
                </button>{" "}
                <button onClick={() => remove(s.id)} title="Löschen">
                  ×
                </button>
              </li>
            ))}
          </ul>
        </div>

        <div>
          {activeId ? (
            <>
              <div
                style={{
                  border: "1px solid #ccc",
                  borderRadius: 6,
                  padding: 12,
                  minHeight: 200,
                  maxHeight: 400,
                  overflowY: "auto",
                }}
              >
                {messages.length === 0 && <p style={{ color: "#888" }}>Noch keine Nachrichten.</p>}
                {messages.map((m, i) => (
                  <div key={i} style={{ marginBottom: 8 }}>
                    <strong style={{ color: m.role === "assistant" ? "#4f46e5" : m.role === "error" ? "crimson" : "#333" }}>
                      {m.role}:
                    </strong>{" "}
                    <span style={{ whiteSpace: "pre-wrap" }}>{m.content}</span>
                  </div>
                ))}
              </div>
              <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.5rem" }}>
                <input
                  style={{ flex: 1 }}
                  placeholder="Nachricht..."
                  value={message}
                  onChange={(e) => setMessage(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && send()}
                />
                <button onClick={send} disabled={busy}>
                  {busy ? "..." : "Senden"}
                </button>
              </div>
            </>
          ) : (
            <p>Session auswählen oder neue erstellen.</p>
          )}
        </div>
      </div>
    </section>
  );
}
