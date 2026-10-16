import { expect, it } from 'vitest'
import { appUrl, httpsStatus, type Https } from '../src/https'

it('uses HTTPS only with a valid certificate for the current hostname', () => {
  const app = {
    domain: 'site.example.com',
    https: {
      enabled: true,
      domain: 'site.example.com',
      expiresAt: Date.now() / 1000 + 86400,
      error: null,
    } as Https,
  }
  expect(appUrl(app)).toBe('https://site.example.com')
  app.https.domain = 'old.example.com'
  expect(appUrl(app)).toBe('http://site.example.com')
  expect(httpsStatus(app.domain, app.https)).toBe('pending')
  app.https.error = 'Issuance failed'
  expect(httpsStatus(app.domain, app.https)).toBe('failed')
  app.https.domain = app.domain
  app.https.expiresAt = 0
  expect(httpsStatus(app.domain, app.https)).toBe('expired')
  expect(appUrl(app)).toBe('http://site.example.com')
  expect(httpsStatus(app.domain)).toBe('disabled')
})
