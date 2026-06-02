export const apiBase = window.location.pathname.startsWith('/agentgrid')
  ? '/agentgrid/api'
  : '/api';

const authStorageKey = 'agentgrid.auth.v1';

export async function fetchJson(path, options) {
  const token = loadStoredAuth()?.token;
  const headers = new Headers(options?.headers || {});
  if (token && !headers.has('authorization')) {
    headers.set('authorization', `Bearer ${token}`);
  }
  const response = await fetch(`${apiBase}${path}`, { ...(options || {}), headers });
  const data = await response.json();
  if (!response.ok || data.ok === false) {
    const error = new Error(data.error?.message || response.statusText);
    error.status = response.status;
    throw error;
  }
  return data;
}

export async function fetchOptionalJson(path, options) {
  try {
    return await fetchJson(path, options);
  } catch (error) {
    if (error.status === 401 || error.status === 403) {
      return { ok: false, items: [] };
    }
    throw error;
  }
}

export function loadStoredAuth() {
  try {
    const raw = window.localStorage.getItem(authStorageKey);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

export function saveStoredAuth(session) {
  try {
    window.localStorage.setItem(authStorageKey, JSON.stringify(session));
  } catch {
    // Ignore storage failures; the in-memory session still works for this page.
  }
}

export function clearStoredAuth() {
  try {
    window.localStorage.removeItem(authStorageKey);
  } catch {
    // Ignore storage failures.
  }
}

export function artifactDownloadUrl(artifact) {
  return `${apiBase}/artifacts/${artifact.metadata.id}/download`;
}
