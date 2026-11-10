import { useEffect, useState } from "react";
import { api, clearToken, getToken, setToken } from "../api";

export default function Login() {
  const [key, setKey] = useState("");
  const [me, setMe] = useState<{ name: string; role: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [authEnabled, setAuthEnabled] = useState<boolean | null>(null);

  useEffect(() => {
    api
      .me()
      .then(setMe)
      .catch(() => setMe(null));
    api
      .health()
      .then(() => setAuthEnabled(true))
      .catch(() => setAuthEnabled(false));
  }, []);

  const login = async () => {
    if (!key) return;
    setError(null);
    try {
      const r = await api.login(key);
      setToken(r.token);
      setMe({ name: r.name, role: r.role });
      setKey("");
    } catch (e) {
      setError(String(e).includes("401") ? "Ungültiger API-Key" : String(e));
    }
  };

  const logout = async () => {
    try {
      await api.logout();
    } finally {
      clearToken();
      setMe(null);
    }
  };

  return (
    <section>
      <h2>Login</h2>
      {me ? (
        <div
          style={{
            background: "#052e16",
            border: "1px solid #16a34a",
            borderRadius: 6,
            padding: 12,
          }}
        >
          <p>
            Angemeldet als <strong>{me.name}</strong> — Rolle:{" "}
            <code>{me.role}</code>
          </p>
          <button onClick={logout}>Abmelden</button>
        </div>
      ) : (
        <div style={{ maxWidth: 480 }}>
          <p>
            Gib deinen API-Key ein, um eine Session zu erzeugen. Der Key bleibt nur lokal im Browser
            (localStorage), der Server speichert nur den Hash.
          </p>
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <input
              style={{ flex: 1 }}
              type="password"
              placeholder="API-Key (bxk-...)"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && login()}
            />
            <button onClick={login}>Anmelden</button>
          </div>
          {error && <p style={{ color: "crimson" }}>{error}</p>}
          {authEnabled === false && (
            <p style={{ color: "#94a3b8" }}>Server nicht erreichbar.</p>
          )}
        </div>
      )}
    </section>
  );
}
