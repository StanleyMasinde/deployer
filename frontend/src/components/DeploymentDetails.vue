<script setup lang="ts">
import type { App, Deployment } from '../api/client'
defineProps<{ deployment?: Deployment; app?: App }>()
</script>
<template>
  <section class="deployment-detail dashboard-panel">
    <RouterLink class="application-back" to="/console/deployments">← Deployments</RouterLink>
    <template v-if="deployment">
      <header class="deployment-detail-heading">
        <h2>{{ app?.domain ?? `Application #${deployment.appId}` }}</h2>
        <span class="deployment-status" :data-status="deployment.status">{{
          deployment.status
        }}</span>
      </header>
      <dl class="detail-values">
        <div>
          <dt>Application</dt>
          <dd>
            <RouterLink :to="`/console/applications/${deployment.appId}`">{{
              app?.domain ?? `Application #${deployment.appId}`
            }}</RouterLink>
          </dd>
        </div>
        <div>
          <dt>Commit</dt>
          <dd>
            <a
              v-if="app"
              :href="`https://github.com/${app.repository}/commit/${deployment.commitSha}`"
              target="_blank"
              rel="noopener noreferrer"
              ><code>{{ deployment.commitSha }}</code></a
            ><code v-else>{{ deployment.commitSha }}</code>
          </dd>
        </div>
        <div>
          <dt>GitHub workflow run</dt>
          <dd>
            <a
              v-if="app"
              :href="`https://github.com/${app.repository}/actions/runs/${deployment.runId}`"
              target="_blank"
              rel="noopener noreferrer"
              >Run #{{ deployment.runId }}</a
            ><span v-else>{{ deployment.runId }}</span> · Attempt {{ deployment.attempt }}
          </dd>
        </div>
        <div>
          <dt>Started</dt>
          <dd>
            <time :datetime="new Date(deployment.createdAt * 1000).toISOString()">{{
              new Date(deployment.createdAt * 1000).toLocaleString()
            }}</time>
          </dd>
        </div>
        <div>
          <dt>Finished</dt>
          <dd>
            {{
              deployment.finishedAt
                ? new Date(deployment.finishedAt * 1000).toLocaleString()
                : 'Not finished'
            }}
          </dd>
        </div>
      </dl>
      <section v-if="deployment.error" class="deployment-error-detail">
        <h3>Failure details</h3>
        <pre>{{ deployment.error }}</pre>
      </section>
    </template>
    <p v-else class="field-hint">
      This deployment could not be loaded. Return to the deployment list and try again.
    </p>
  </section>
</template>
