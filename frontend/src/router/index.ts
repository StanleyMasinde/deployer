import { nextTick } from 'vue'
import type { Router, RouteRecordRaw } from 'vue-router'
import { handleHotUpdate } from 'vue-router/auto-routes'
import { request, type App, type Deployment, type Repository } from '../api/client'
import { loadConsoleData } from './data'
import { navigationPending } from './loading'
import { routes as generatedRoutes } from 'vue-router/auto-routes'
const formRoute = () => import('../components/ApplicationFormRoute.vue')
function withModalRoutes(record: RouteRecordRaw): RouteRecordRaw {
  if (record.path === '/console/applications')
    return {
      ...record,
      children: [
        {
          path: 'new',
          component: formRoute,
          meta: { modal: 'new' as const, title: 'New application · Deployer', ssg: false },
        },
      ],
    }
  if (record.path.startsWith('/console/applications/:appId'))
    return {
      ...record,
      children: [
        {
          path: 'edit',
          component: formRoute,
          meta: { modal: 'edit' as const, title: 'Edit application · Deployer', ssg: false },
        },
        {
          path: 'delete',
          component: () => import('../components/DeleteApplicationRoute.vue'),
          meta: { modal: 'delete' as const, title: 'Delete application · Deployer', ssg: false },
        },
      ],
    }
  return record
}
export const routes = generatedRoutes.map(withModalRoutes)

export function registerRouter(router: Router) {
  let controller: AbortController | undefined
  let target = ''
  if (!import.meta.env.SSR) {
    router.beforeEach((to, from) => {
      controller?.abort()
      controller = new AbortController()
      target = to.fullPath
      navigationPending.value = to.path !== from.path
    })
    router.beforeResolve(async (to, from) => {
      if (to.path === from.path && from.meta.consoleData) {
        to.meta.consoleData = from.meta.consoleData
        to.meta.deploymentData = from.meta.deploymentData
        to.meta.applicationData = from.meta.applicationData
        to.meta.applicationDeployments = from.meta.applicationDeployments
        to.meta.repositories = from.meta.repositories
        return
      }
      const signal = controller!.signal
      try {
        if (to.path.startsWith('/console')) {
          const data = await loadConsoleData(signal)
          if (signal.aborted) return false
          if (data === 'setup') return '/setup'
          to.meta.consoleData = data
          if (data.authenticated) {
            try {
              const deploymentId = 'deploymentId' in to.params ? to.params.deploymentId : undefined
              if (deploymentId) {
                to.meta.deploymentData = await request<Deployment>(`/deployments/${deploymentId}`, {
                  signal,
                })
                to.meta.applicationData = data.apps.find(
                  (app) => app.id === to.meta.deploymentData?.appId,
                )
              }
              const appId = 'appId' in to.params ? to.params.appId : undefined
              if (appId) {
                ;[to.meta.applicationData, to.meta.applicationDeployments] = await Promise.all([
                  request<App>(`/apps/${appId}`, { signal }),
                  request<Deployment[]>(`/apps/${appId}/deployments`, { signal }),
                ])
              }
              if (to.meta.modal === 'new' || to.meta.modal === 'edit')
                to.meta.repositories = await request<Repository[]>('/repositories', { signal })
            } catch (cause) {
              if (signal.aborted) return false
              data.error = cause instanceof Error ? cause.message : 'Could not load application.'
            }
          }
        } else if (to.path === '/setup') {
          try {
            const setup = await request<{ required: boolean }>('/setup', { signal })
            if (!setup.required) return '/console'
            to.meta.setupError = ''
          } catch (cause) {
            if (signal.aborted) return false
            to.meta.setupError =
              cause instanceof Error ? cause.message : 'Could not check setup status.'
          }
        }
      } catch (cause) {
        if (signal.aborted) return false
        throw cause
      }
    })
    router.onError(() => {
      navigationPending.value = false
    })
  }
  router.afterEach(async (to, from, failure) => {
    if (import.meta.env.SSR) return
    if (target === to.fullPath) navigationPending.value = false
    if (failure) return
    document.title =
      to.meta.consoleData?.authenticated === false
        ? 'Sign in · Deployer'
        : (to.meta.title ?? 'Deployer')
    if (from.matched.length && to.path !== from.path && !to.meta.modal && !from.meta.modal) {
      await nextTick()
      const heading = document.querySelector<HTMLElement>('main h1')
      heading?.setAttribute('tabindex', '-1')
      heading?.focus({ preventScroll: true })
    }
  })
  if (import.meta.hot && !import.meta.env.SSR) {
    handleHotUpdate(router, (updatedRoutes) => {
      router.clearRoutes()
      for (const record of updatedRoutes) router.addRoute(withModalRoutes(record))
    })
  }
}
