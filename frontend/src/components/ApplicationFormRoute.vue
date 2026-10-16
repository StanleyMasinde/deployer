<script setup lang="ts">
import { inject } from 'vue'
import { applicationFormKey } from './applicationFormContext'
import {
  Label,
  DialogRoot,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  DialogClose,
} from 'reka-ui'
import UiIcon from './ui/UiIcon.vue'
import UiInput from './ui/UiInput.vue'
import UiButton from './ui/UiButton.vue'
import UiSelect from './ui/UiSelect.vue'
import UiCheckbox from './ui/UiCheckbox.vue'
const {
  form,
  fieldErrors,
  editing,
  busy,
  error,
  loadingRepositories,
  repositoryOptions,
  repositories,
  addApp,
  chooseRepository,
  changeEcosystem,
  changePackageManager,
  closeModal,
  restoreFormFocus,
} = inject(applicationFormKey)!
</script>
<template>
  <DialogRoot :open="true" @update:open="!$event && closeModal()"
    ><DialogPortal
      ><DialogOverlay class="console-dialog-overlay" /><DialogContent
        class="console-modal"
        @interact-outside="busy && $event.preventDefault()"
        @escape-key-down="busy && $event.preventDefault()"
        @close-auto-focus="restoreFormFocus"
      >
        <div class="modal-heading">
          <span class="modal-icon"><UiIcon name="repo" /></span
          ><DialogClose as-child
            ><UiButton :disabled="busy" aria-label="Close new application"
              ><UiIcon name="close" /></UiButton
          ></DialogClose>
        </div>
        <DialogTitle class="modal-title">{{
          editing === null ? 'Connect a repository' : 'Edit application'
        }}</DialogTitle
        ><DialogDescription class="modal-description">{{
          editing === null
            ? 'Give your next app a home. We’ll open a pull request with your build workflow for you to review.'
            : 'Update your source, build, and destination settings. Build changes require an updated workflow PR.'
        }}</DialogDescription>
        <p v-if="error" class="console-message is-error" role="alert">{{ error }}</p>
        <form novalidate class="modal-form" @submit.prevent="addApp">
          <fieldset :disabled="busy">
            <legend>Source</legend>
            <Label for="repository">Repository</Label
            ><UiSelect
              id="repository"
              :error="fieldErrors['repository']"
              v-model="form.repository"
              name="repository"
              :options="repositoryOptions"
              :placeholder="loadingRepositories ? 'Loading repositories…' : 'Select a repository'"
              :disabled="busy || loadingRepositories"
              @update:model-value="chooseRepository"
            />
            <p v-if="!loadingRepositories && !repositories.length" class="field-hint">
              No repositories found. Install your GitHub App on a repository, then reopen this form.
            </p>
            <Label for="branch">Production branch</Label
            ><UiInput
              id="branch"
              :error="fieldErrors['branch']"
              v-model="form.branch"
              :disabled="busy"
            />
            <p class="field-hint">Pushes to this branch trigger a deployment.</p>
          </fieldset>
          <fieldset :disabled="busy">
            <legend>Build setup</legend>
            <Label for="deployment-type">Deployment type</Label>
            <UiSelect
              id="deployment-type"
              :error="fieldErrors['buildConfig.deploymentType']"
              v-model="form.buildConfig.deploymentType"
              :options="[{ value: 'static', label: 'Static site' }]"
              :disabled="busy"
            />
            <p class="field-hint">Serve HTML, CSS, and JavaScript from your VPS.</p>
            <Label for="ecosystem">Build ecosystem</Label>
            <UiSelect
              id="ecosystem"
              :error="fieldErrors['buildConfig.ecosystem']"
              :model-value="form.buildConfig.ecosystem"
              :options="[
                { value: 'none', label: 'None — plain static files' },
                { value: 'javascript', label: 'JavaScript' },
              ]"
              :disabled="busy"
              @update:model-value="changeEcosystem"
            />
            <p class="field-hint">Choose how your site is built.</p>
            <template v-if="form.buildConfig.ecosystem === 'javascript'">
              <Label for="package-manager">Package manager</Label>
              <UiSelect
                id="package-manager"
                :error="fieldErrors['buildConfig.packageManager']"
                :model-value="form.buildConfig.packageManager ?? 'npm'"
                :options="[
                  { value: 'npm', label: 'npm (Node.js 24)' },
                  { value: 'pnpm', label: 'pnpm 12' },
                  { value: 'yarn', label: 'Yarn 4' },
                  { value: 'bun', label: 'Bun 1' },
                ]"
                :disabled="busy"
                @update:model-value="changePackageManager"
              />
              <p class="field-hint">
                Installs the selected tool in your GitHub workflow. Commit its lockfile before
                building.
              </p>
            </template>
            <Label for="framework">Framework preset</Label>
            <UiSelect
              id="framework"
              :error="fieldErrors['buildConfig.framework']"
              v-model="form.buildConfig.framework"
              :options="
                form.buildConfig.ecosystem === 'none'
                  ? [{ value: 'plainHtml', label: 'Plain HTML / CSS / JavaScript' }]
                  : [{ value: 'vite', label: 'Vite' }]
              "
              :disabled="busy"
            />
            <p class="field-hint">
              {{
                form.buildConfig.ecosystem === 'none'
                  ? 'Publish files as they are, with no runtime or build step.'
                  : 'Build with Vite, then publish the output directory. Commands remain editable.'
              }}
            </p>
            <div v-if="form.buildConfig.ecosystem === 'javascript'" class="modal-field-grid">
              <div>
                <Label for="install">Install command</Label
                ><UiInput
                  id="install"
                  :error="fieldErrors['installCommand']"
                  v-model="form.installCommand"
                  :disabled="busy"
                />
              </div>
              <div>
                <Label for="build">Build command</Label
                ><UiInput
                  id="build"
                  :error="fieldErrors['buildCommand']"
                  v-model="form.buildCommand"
                  :disabled="busy"
                />
              </div>
            </div>
            <Label for="output">Output directory</Label
            ><UiInput
              id="output"
              :error="fieldErrors['outputDirectory']"
              v-model="form.outputDirectory"
              :disabled="busy"
            />
            <p class="field-hint">
              {{
                form.buildConfig.ecosystem === 'none'
                  ? 'The folder containing index.html. Use . for the repository root.'
                  : 'The folder containing your built index.html.'
              }}
            </p>
          </fieldset>
          <fieldset :disabled="busy">
            <legend>Destination</legend>
            <Label for="domain">Domain</Label
            ><UiInput
              id="domain"
              :error="fieldErrors['domain']"
              v-model="form.domain"
              placeholder="www.example.com"
              :disabled="busy"
              autocapitalize="none"
            />
            <p class="field-hint">
              Point DNS at your VPS. Enable HTTPS in application details. Domain changes require a
              certificate for the new hostname.
            </p>
            <Label for="spa" class="checkbox"
              ><UiCheckbox id="spa" v-model="form.spa" :disabled="busy" />Enable SPA routing</Label
            >
            <p class="field-hint">Serve index.html for routes that don’t match a file.</p>
          </fieldset>
          <div class="modal-actions">
            <DialogClose as-child><UiButton :disabled="busy">Cancel</UiButton></DialogClose
            ><UiButton
              type="submit"
              class="primary"
              :disabled="busy || loadingRepositories || !form.repository"
              >{{
                busy
                  ? editing === null
                    ? 'Opening PR…'
                    : 'Saving…'
                  : editing === null
                    ? 'Create workflow PR'
                    : 'Save settings'
              }}<UiIcon name="arrow"
            /></UiButton>
          </div>
        </form> </DialogContent></DialogPortal
  ></DialogRoot>
</template>
