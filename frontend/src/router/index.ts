import type { Router } from 'vue-router'
import { handleHotUpdate } from 'vue-router/auto-routes'

export { routes } from 'vue-router/auto-routes'

export function registerRouter(router: Router) {
  router.afterEach((to) => {
    if (!import.meta.env.SSR) document.title = to.meta.title ?? 'Deployer'
  })

  if (import.meta.hot && !import.meta.env.SSR) handleHotUpdate(router)
}
