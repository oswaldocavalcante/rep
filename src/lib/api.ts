/**
 * Cliente HTTP para comunicação com o rep-server.
 * No modo Tauri (desktop), a base URL é http://localhost:3001.
 * Na UI web servida pelo LXC, chamadas relativas funcionam diretamente.
 */

const STORAGE_KEY = "rep_token";

// Na web servida pelo LXC (porta 3001) ou pelo proxy Vite em dev, caminhos relativos funcionam.
const BASE_URL = "";

export function getToken(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string): void {
  localStorage.setItem(STORAGE_KEY, token);
}

export function clearToken(): void {
  localStorage.removeItem(STORAGE_KEY);
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function apiFetch<T = unknown>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const token = getToken();
  const headers: HeadersInit = {
    "Content-Type": "application/json",
    ...(options.headers ?? {}),
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };

  const resp = await fetch(`${BASE_URL}${path}`, { ...options, headers });

  if (resp.status === 401) {
    clearToken();
    // Dispara evento para que App.tsx redirecione ao login
    window.dispatchEvent(new Event("rep:unauthorized"));
    throw new ApiError(401, "Não autenticado");
  }

  if (!resp.ok) {
    let message = `HTTP ${resp.status}`;
    try {
      const json = await resp.json();
      message = json.error ?? message;
    } catch {
      // ignora
    }
    throw new ApiError(resp.status, message);
  }

  // 204 No Content → retorna undefined
  if (resp.status === 204) {
    return undefined as unknown as T;
  }

  return resp.json() as Promise<T>;
}

// ─── Helpers por recurso ─────────────────────────────────────────────────────

export const api = {
  // Auth
  login: (password: string) =>
    apiFetch<{ token: string }>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ password }),
    }),

  logout: () => apiFetch("/auth/logout", { method: "POST" }),

  me: () => apiFetch<{ authenticated: boolean }>("/auth/me"),

  changePassword: (currentPassword: string, newPassword: string) =>
    apiFetch("/api/auth/password", {
      method: "PUT",
      body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
    }),

  // Config
  getConfig: () => apiFetch<Record<string, unknown>>("/api/config"),

  saveConfig: (data: Record<string, unknown>) =>
    apiFetch("/api/config", {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  testConnection: (body: {
    device_ip: string;
    device_user: string;
    device_password: string;
  }) =>
    apiFetch<{ success: boolean; error?: string }>("/api/test-connection", {
      method: "POST",
      body: JSON.stringify(body),
    }),

  provision: (body: {
    api_key: string;
    clock_id: string;
  }) =>
    apiFetch<{ success: boolean; ipAddress?: string; error?: string }>("/api/provision", {
      method: "POST",
      body: JSON.stringify(body),
    }),

  // Status / Sync
  getStatus: () => apiFetch<Record<string, unknown>>("/api/status"),

  syncNow: () =>
    apiFetch<{ success: boolean; records_sent: number; message: string }>("/api/sync/run", {
      method: "POST",
    }),

  reprocessHistory: () =>
    apiFetch<{ success: boolean; records_sent: number; message: string }>("/api/sync/reprocess", {
      method: "POST",
    }),

  resetSync: () => apiFetch("/api/sync/reset", { method: "POST" }),

  // Logs
  getLogs: () =>
    apiFetch<
      { id: number; timestamp: string; status: string; records_sent: number; message: string }[]
    >("/api/logs"),
};
