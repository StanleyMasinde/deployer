import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { liveConnected, liveSnapshot, startLiveUpdates, stopLiveUpdates } from '../src/live'

class TestEventSource {
  static instances: TestEventSource[] = []
  listeners = new Map<string, (event: { data: string }) => void>()
  onopen?: () => void
  onerror?: () => void
  closed = false
  constructor(public url: string) {
    TestEventSource.instances.push(this)
  }
  addEventListener(name: string, callback: (event: { data: string }) => void) {
    this.listeners.set(name, callback)
  }
  close() {
    this.closed = true
  }
  snapshot(data: string) {
    this.listeners.get('snapshot')?.({ data })
  }
}

beforeEach(() => {
  TestEventSource.instances = []
  vi.stubGlobal('EventSource', TestEventSource)
})
afterEach(() => {
  stopLiveUpdates()
  vi.unstubAllGlobals()
})

it('shares one stream across consumers and clears private state when stopped', () => {
  startLiveUpdates()
  startLiveUpdates()
  expect(TestEventSource.instances).toHaveLength(1)
  const source = TestEventSource.instances[0]!
  expect(source.url).toBe('/api/events')
  source.onopen?.()
  source.snapshot(JSON.stringify({ apps: [{ id: 1 }], deployments: [] }))
  expect(liveConnected.value).toBe(true)
  expect(liveSnapshot.value?.apps).toEqual([{ id: 1 }])
  stopLiveUpdates()
  expect(source.closed).toBe(true)
  expect(liveConnected.value).toBe(false)
  expect(liveSnapshot.value).toBeUndefined()
  startLiveUpdates()
  expect(TestEventSource.instances).toHaveLength(2)
})

it('keeps the last valid snapshot through malformed events and native reconnection', () => {
  startLiveUpdates()
  const source = TestEventSource.instances[0]!
  source.onopen?.()
  const snapshot = { apps: [{ id: 1 }], deployments: [{ id: 2, status: 'building' }] }
  source.snapshot(JSON.stringify(snapshot))
  source.snapshot('invalid json')
  source.snapshot(JSON.stringify({ apps: {}, deployments: [] }))
  expect(liveSnapshot.value).toEqual(snapshot)
  source.onerror?.()
  expect(liveConnected.value).toBe(false)
  expect(liveSnapshot.value).toEqual(snapshot)
  source.onopen?.()
  expect(liveConnected.value).toBe(true)
  expect(TestEventSource.instances).toHaveLength(1)
  source.snapshot(JSON.stringify({ apps: [], deployments: [] }))
  expect(liveSnapshot.value).toEqual({ apps: [], deployments: [] })
})

it('does not open a stream when EventSource is unavailable', () => {
  vi.stubGlobal('EventSource', undefined)
  expect(() => startLiveUpdates()).not.toThrow()
  expect(TestEventSource.instances).toHaveLength(0)
})
