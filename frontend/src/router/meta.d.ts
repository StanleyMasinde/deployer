import 'vue-router'
import type { App, Deployment, Repository } from '../api/client'
import type { ConsoleData } from './data'

export {}

declare module 'vue-router' {
  interface RouteMeta {
    ssg?: boolean
    auth?: boolean
    title?: string
    consoleData?: ConsoleData
    setupError?: string
    deploymentData?: Deployment
    applicationData?: App
    applicationDeployments?: Deployment[]
    repositories?: Repository[]
    modal?: 'new' | 'edit' | 'delete'
  }
}
