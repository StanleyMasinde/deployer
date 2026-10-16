import { afterEach, expect, it, vi } from 'vitest'
import { body, request } from '../src/api/client'

afterEach(() => vi.unstubAllGlobals())

it('sends and reads JSON:API documents', async () => {
  const fetch = vi.fn().mockResolvedValue(
    new Response(
      JSON.stringify({
        data: [{ type: 'apps', id: '1', attributes: { domain: 'example.com' } }],
      }),
    ),
  )
  vi.stubGlobal('fetch', fetch)

  expect(await request('/apps')).toEqual([{ id: 1, domain: 'example.com' }])
  expect(fetch.mock.calls[0][1].headers.Accept).toBe('application/vnd.api+json')
  expect(JSON.parse(body('apps', { domain: 'example.com' })).data.type).toBe('apps')
})

it('surfaces protocol errors and supports empty responses', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          errors: [{ status: '401', detail: 'Sign in' }],
        }),
        { status: 401 },
      ),
    ),
  )

  await expect(request('/session')).rejects.toMatchObject({ message: 'Sign in', status: 401 })

  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 204 })))
  expect(await request('/session', { method: 'DELETE' })).toBeUndefined()
})

it('accepts queued application deletion without a JSON response', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 202 })))
  expect(await request('/apps/1', { method: 'DELETE' })).toBeUndefined()
})
