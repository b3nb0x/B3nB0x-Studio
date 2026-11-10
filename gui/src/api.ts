const BASE = "";

async function req(path: string, init?: RequestInit): Promise<any> {
  const resp = await fetch(`${BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!resp.ok) throw new Error(`API-Fehler ${resp.status}: ${await resp.text()}`);
  return resp.json();
}

export const api = {
  health: () => req("/health"),
  models: () => req("/v1/models"),
  routing: () => req("/v1/routing"),
  setRoute: (key: string, backend: string, model: string) =>
    req(`/v1/routing/${key}`, { method: "PUT", body: JSON.stringify({ backend, model }) }),
  chat: (route: string, messages: { role: string; content: string }[]) =>
    req("/v1/chat", { method: "POST", body: JSON.stringify({ route, messages }) }),
  startRun: (task: string) =>
    req("/v1/pipeline/runs", { method: "POST", body: JSON.stringify({ task }) }),
  listRuns: () => req("/v1/pipeline/runs"),
  switchPhase: (phase: string, backend: string, model: string) =>
    req("/v1/pipeline/switch", { method: "POST", body: JSON.stringify({ phase, backend, model }) }),
  listTasks: () => req("/v1/tasks"),
  createTask: (task: Record<string, unknown>) =>
    req("/v1/tasks", { method: "POST", body: JSON.stringify(task) }),
  runTask: (id: string) => req(`/v1/tasks/${id}/run`, { method: "POST", body: "{}" }),
};
