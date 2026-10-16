import { afterEach, expect, it, vi } from 'vitest'
import { loadConsoleData } from '../src/router/data'

afterEach(() => vi.unstubAllGlobals())

function response(attributes: object, status = 200) {
  return new Response(JSON.stringify({ data: { type: 'test', attributes } }), { status })
}

it('redirects incomplete setup without requesting protected resources', async () => {
  const fetch = vi.fn().mockResolvedValue(response({ required: true }))
  vi.stubGlobal('fetch', fetch)
  expect(await loadConsoleData(new AbortController().signal)).toBe('setup')
  expect(fetch).toHaveBeenCalledTimes(1)
})

it('returns a sign-in state when the session has expired', async () => {
  const fetch = vi
    .fn()
    .mockResolvedValueOnce(response({ required: false }))
    .mockResolvedValueOnce(
      new Response(JSON.stringify({ errors: [{ detail: 'Sign in' }] }), { status: 401 }),
    )
  vi.stubGlobal('fetch', fetch)
  expect(await loadConsoleData(new AbortController().signal)).toEqual({
    authenticated: false,
    apps: [],
    deployments: [],
    error: '',
  })
  expect(fetch).toHaveBeenCalledTimes(2)
})

it('waits for both inventories and passes cancellation to every request', async () => {
  let releaseDeployments!: (value: Response) => void
  const deployments = new Promise<Response>((resolve) => {
    releaseDeployments = resolve
  })
  const fetch = vi
    .fn()
    .mockResolvedValueOnce(response({ required: false }))
    .mockResolvedValueOnce(response({ authenticated: true }))
    .mockResolvedValueOnce(
      new Response(
        JSON.stringify({ data: [{ type: 'apps', id: '7', attributes: { domain: 'site.test' } }] }),
      ),
    )
    .mockReturnValueOnce(deployments)
  vi.stubGlobal('fetch', fetch)
  const signal = new AbortController().signal
  let finished = false
  const pending = loadConsoleData(signal).then((data) => {
    finished = true
    return data
  })
  await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(4))
  expect(finished).toBe(false)
  releaseDeployments(new Response(JSON.stringify({ data: [] })))
  expect(await pending).toMatchObject({
    authenticated: true,
    apps: [{ id: 7, domain: 'site.test' }],
    deployments: [],
    error: '',
  })
  for (const [, options] of fetch.mock.calls) expect(options.signal).toBe(signal)
})

it('propagates cancellation instead of rendering an error for an abandoned navigation', async () => {
  const controller = new AbortController()
  const failure = new DOMException('Aborted', 'AbortError')
  vi.stubGlobal(
    'fetch',
    vi.fn().mockImplementation(() => {
      controller.abort()
      return Promise.reject(failure)
    }),
  )
  await expect(loadConsoleData(controller.signal)).rejects.toBe(failure)
})

it('returns an inline load error for a network failure', async () => {
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('Network unavailable')))
  expect(await loadConsoleData(new AbortController().signal)).toMatchObject({
    error: 'Network unavailable',
  })
})
