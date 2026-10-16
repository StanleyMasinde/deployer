import type { InjectionKey, Ref } from 'vue'
import type { Repository } from '../api/client'
import type { BuildConfig } from '../build'
export type ApplicationFormContext = {
  form: {
    repository: string
    branch: string
    domain: string
    buildConfig: BuildConfig
    installCommand: string
    buildCommand: string
    outputDirectory: string
    spa: boolean
  }
  fieldErrors: Ref<Record<string, string>>
  editing: Ref<number | null>
  busy: Ref<boolean>
  error: Ref<string>
  loadingRepositories: Ref<boolean>
  repositoryOptions: Ref<{ value: string; label: string }[]>
  repositories: Ref<Repository[]>
  addApp: () => Promise<void>
  chooseRepository: () => void
  changeEcosystem: (value: string) => void
  changePackageManager: (value: string) => void
  closeModal: () => void
  restoreFormFocus: (event: Event) => void
}
export const applicationFormKey: InjectionKey<ApplicationFormContext> = Symbol('application-form')
