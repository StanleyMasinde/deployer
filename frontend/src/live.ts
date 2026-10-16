import { shallowRef } from 'vue'
import type { App, Deployment } from './api/client'
export const liveSnapshot = shallowRef<{ apps: App[]; deployments: Deployment[] }>()
export const liveConnected = shallowRef(false)
let source: EventSource | undefined
export function startLiveUpdates() {
  if (source || typeof EventSource === 'undefined') return
  source = new EventSource('/api/events')
  source.onopen = () => {
    liveConnected.value = true
  }
  source.onerror = () => {
    liveConnected.value = false
  }
  source.addEventListener('snapshot', (event) => {
    try {
      const data = JSON.parse((event as MessageEvent<string>).data)
      if (Array.isArray(data.apps) && Array.isArray(data.deployments)) liveSnapshot.value = data
    } catch {
      /* A subsequent SSE snapshot will restore state. */
    }
  })
}
export function stopLiveUpdates() {
  source?.close()
  source = undefined
  liveConnected.value = false
  liveSnapshot.value = undefined
}
