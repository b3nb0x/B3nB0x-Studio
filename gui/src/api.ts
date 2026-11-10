const BASE = "";

export function getToken(): string | null {
  return localStorage.getItem("b3nbox_token");
}

export function setToken(token: string) {
  localStorage.setItem("b3nbox_token", token);
}

export function clearToken() {
  localStorage.removeItem("b3nbox_token");
}

async function req(path: string, init?: RequestInit): Promise<any> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = getToken();
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const resp = await fetch(`${BASE}${path}`, { headers, ...init });
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
  searchMemory: (q: string) => req(`/v1/memory?q=${encodeURIComponent(q)}`),
  recentMemory: (limit = 10) => req(`/v1/memory/recent?limit=${limit}`),
  addMemory: (kind: string, key: string, content: string) =>
    req("/v1/memory", { method: "POST", body: JSON.stringify({ kind, key, content }) }),
  permissions: () => req("/v1/permissions"),
  addPermission: (rule: { effect: string; action: string; scope?: string }) =>
    req("/v1/permissions", { method: "POST", body: JSON.stringify(rule) }),
  checkPermission: (action: string, scope?: string) =>
    req("/v1/permissions/check", { method: "POST", body: JSON.stringify({ action, scope }) }),
  mcpTools: () => req("/v1/mcp/tools"),
  mcpCall: (server: string, tool: string, args: unknown) =>
    req("/v1/mcp/call", { method: "POST", body: JSON.stringify({ server, tool, arguments: args }) }),
  agentRun: (task: string, route: string, maxIterations: number) =>
    req("/v1/agent/run", {
      method: "POST",
      body: JSON.stringify({ task, route, max_iterations: maxIterations }),
    }),
  listScripts: () => req("/v1/scripts"),
  registerScript: (spec: { name: string; command: string; args: string[]; description: string }) =>
    req("/v1/scripts", { method: "POST", body: JSON.stringify(spec) }),
  runScript: (name: string, approval: boolean) =>
    req(`/v1/scripts/${name}/run`, { method: "POST", body: JSON.stringify({ approval }) }),
  listSchedule: () => req("/v1/schedule"),
  addSchedule: (entry: { name: string; script: string; args: string[]; every_seconds: number; approval_required: boolean }) =>
    req("/v1/schedule", { method: "POST", body: JSON.stringify(entry) }),
  removeSchedule: (name: string) =>
    req(`/v1/schedule?name=${encodeURIComponent(name)}`, { method: "DELETE" }),
  searchSemantic: (q: string) =>
    req(`/v1/memory/search-semantic?q=${encodeURIComponent(q)}`, { method: "POST" }),
  listSessions: () => req("/v1/sessions"),
  createSession: (name: string) =>
    req("/v1/sessions", { method: "POST", body: JSON.stringify({ name }) }),
  getSession: (id: string) => req(`/v1/sessions/${id}`),
  deleteSession: (id: string) => req(`/v1/sessions/${id}`, { method: "DELETE" }),
  sessionChat: (id: string, message: string) =>
    req(`/v1/sessions/${id}/chat`, { method: "POST", body: JSON.stringify({ message }) }),
  recentAudit: (limit: number) => req(`/v1/audit?limit=${limit}`),
  guardBlocked: () => req("/v1/guard/blocked"),
  login: (key: string) =>
    req("/v1/auth/login", { method: "POST", body: JSON.stringify({ key }) }),
  logout: () => req("/v1/auth/logout", { method: "POST", body: "{}" }),
  me: () => req("/v1/auth/me"),
};
