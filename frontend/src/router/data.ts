import { ApiError, request, type App, type Deployment } from '../api/client'
export type ConsoleData = {
  authenticated: boolean
  user?: { id: number; name: string; email: string }
  apps: App[]
  deployments: Deployment[]
  error: string
}
export async function loadConsoleData(signal: AbortSignal): Promise<ConsoleData | 'setup'> {
  const data: ConsoleData = { authenticated: false, apps: [], deployments: [], error: '' }
  try {
    const setup = await request<{ required: boolean }>('/setup', { signal })
    if (setup.required) return 'setup'
    const session = await request<{ user?: ConsoleData['user'] }>('/session', { signal })
    data.user = session.user
    data.authenticated = true
    ;[data.apps, data.deployments] = await Promise.all([
      request<App[]>('/apps', { signal }),
      request<Deployment[]>('/deployments', { signal }),
    ])
  } catch (cause) {
    if (signal.aborted) throw cause
    if (cause instanceof ApiError && cause.status === 401) data.authenticated = false
    else data.error = cause instanceof Error ? cause.message : 'Could not load the workspace.'
  }
  return data
}
