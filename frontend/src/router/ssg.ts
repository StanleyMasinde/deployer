import type { RouteRecordRaw } from 'vue-router'

export function prerenderedRoutes(paths: string[], routes: readonly RouteRecordRaw[]): string[] {
  const allowed = new Set<string>()

  function walk(records: readonly RouteRecordRaw[], parent = '') {
    for (const route of records) {
      const fullPath = route.path.startsWith('/')
        ? route.path
        : `${parent.replace(/\/$/, '')}/${route.path}`
      const path = fullPath.replace(/\/$/, '') || '/'

      if (route.meta?.ssg) allowed.add(path)
      if (route.children) walk(route.children, path)
    }
  }

  walk(routes)

  return [...new Set(paths.map((path) => (path.startsWith('/') ? path : `/${path}`)))].filter(
    (path) => allowed.has(path),
  )
}
