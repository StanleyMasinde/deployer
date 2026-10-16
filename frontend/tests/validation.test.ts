import { expect, it } from 'vitest'
import { signInSchema, setupSchema, applicationSchema, deletionSchema } from '../src/validation'
import { buildPreset } from '../src/build'

it('rejects empty credentials and invalid emails without imposing setup password length at sign-in', () => {
  expect(signInSchema.safeParse({ email: 'invalid', password: '' }).success).toBe(false)
  expect(signInSchema.safeParse({ email: 'owner@example.com', password: 'legacy' }).success).toBe(
    true,
  )
})
it('validates setup password length and confirmation', () => {
  const valid = {
    code: 'setup-code',
    name: 'Owner',
    email: 'owner@example.com',
    password: 'long-password-for-test',
    confirmation: 'long-password-for-test',
  }
  expect(setupSchema.safeParse(valid).success).toBe(true)
  expect(
    setupSchema.safeParse({ ...valid, password: 'short', confirmation: 'short' }).success,
  ).toBe(false)
  const mismatch = setupSchema.safeParse({ ...valid, confirmation: 'different' })
  expect(mismatch.success).toBe(false)
  if (!mismatch.success) expect(mismatch.error.issues[0]?.path).toEqual(['confirmation'])
})
it('checks domains, traversal, and ecosystem-specific commands', () => {
  const valid = {
    repository: 'owner/site',
    domain: 'site.example.com',
    branch: 'main',
    spa: false,
    ...buildPreset('none'),
  }
  expect(applicationSchema.safeParse(valid).success).toBe(true)
  expect(
    applicationSchema.safeParse({ ...valid, domain: 'https://site.example.com' }).success,
  ).toBe(false)
  expect(applicationSchema.safeParse({ ...valid, outputDirectory: '../dist' }).success).toBe(false)
  expect(applicationSchema.safeParse({ ...valid, buildCommand: 'npm run build' }).success).toBe(
    false,
  )
  expect(
    applicationSchema.safeParse({ ...valid, ...buildPreset('javascript'), installCommand: '' })
      .success,
  ).toBe(false)
})
it('requires an exact domain for deletion', () => {
  expect(
    deletionSchema('site.example.com').safeParse({ confirmation: 'other.example.com' }).success,
  ).toBe(false)
  expect(
    deletionSchema('site.example.com').safeParse({ confirmation: 'site.example.com' }).success,
  ).toBe(true)
})

it('validates reset email, token, and matching passwords with Zod', async () => {
  const { forgotPasswordSchema, resetPasswordSchema } = await import('../src/validation')
  expect(forgotPasswordSchema.safeParse({ email: ' owner@example.com ' })).toMatchObject({
    success: true,
    data: { email: 'owner@example.com' },
  })
  expect(forgotPasswordSchema.safeParse({ email: 'invalid' }).success).toBe(false)
  const valid = {
    token: 'a'.repeat(64),
    password: 'a unique long password',
    confirmation: 'a unique long password',
  }
  expect(resetPasswordSchema.safeParse(valid).success).toBe(true)
  expect(resetPasswordSchema.safeParse({ ...valid, token: 'invalid' }).success).toBe(false)
  const mismatch = resetPasswordSchema.safeParse({ ...valid, confirmation: 'different' })
  expect(mismatch.success).toBe(false)
  if (!mismatch.success) expect(mismatch.error.issues[0]?.path).toEqual(['confirmation'])
})

it('counts password characters and UTF-8 bytes consistently for setup and recovery', async () => {
  const { resetPasswordSchema } = await import('../src/validation')
  for (const [password, valid] of [
    ['a'.repeat(14), false],
    ['a'.repeat(15), true],
    ['🦀'.repeat(14), false],
    ['🦀'.repeat(15), true],
    ['🦀'.repeat(256), true],
    ['🦀'.repeat(257), false],
    ['a'.repeat(1025), false],
  ] as const) {
    expect(
      resetPasswordSchema.safeParse({ token: 'a'.repeat(64), password, confirmation: password })
        .success,
    ).toBe(valid)
    expect(
      setupSchema.safeParse({
        code: 'setup-code',
        name: 'Owner',
        email: 'owner@example.com',
        password,
        confirmation: password,
      }).success,
    ).toBe(valid)
  }
})
