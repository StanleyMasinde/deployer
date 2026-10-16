<script setup lang="ts">
import { computed } from 'vue'
import type { App, Deployment } from '../api/client'
import UiIcon from './ui/UiIcon.vue'
import UiButton from './ui/UiButton.vue'
const props = defineProps<{ apps: App[]; deployments: Deployment[] }>()
defineEmits<{ details: [app: App]; create: [] }>()
const ordered = computed(() =>
  [...props.deployments].sort((a, b) => b.createdAt - a.createdAt || b.id - a.id),
)
const recent = computed(() => ordered.value.slice(0, 6))
const latest = (id: number) => ordered.value.find((d) => d.appId === id)
const live = computed(
  () => props.apps.filter((a) => !a.deletionRequested && latest(a.id)?.status === 'live').length,
)
const working = computed(
  () =>
    props.deployments.filter((d) => ['queued', 'building', 'deploying'].includes(d.status)).length,
)
const attention = computed(() =>
  props.apps.filter(
    (a) => !a.deletionRequested && (!latest(a.id) || latest(a.id)?.status === 'failed'),
  ),
)
const name = (id: number) => props.apps.find((a) => a.id === id)?.domain ?? `Application #${id}`
const label = (status: string) =>
  ({ live: 'Live', failed: 'Failed', deploying: 'Deploying', queued: 'Queued' })[status] ?? status
</script>
<template>
  <div class="dashboard">
    <dl class="dashboard-metrics" aria-label="Instance overview">
      <div>
        <dt><UiIcon name="apps" />Applications</dt>
        <dd>{{ apps.length }}</dd>
        <span>Connected repositories</span>
      </div>
      <div>
        <dt><span class="state-dot is-live"></span>Live</dt>
        <dd>{{ live }}</dd>
        <span>Latest release published</span>
      </div>
      <div>
        <dt><span class="state-dot is-working"></span>In progress</dt>
        <dd>{{ working }}</dd>
        <span>Queued or deploying</span>
      </div>
      <div>
        <dt><span class="state-dot is-waiting"></span>Needs attention</dt>
        <dd>{{ attention.length }}</dd>
        <span>Failed or awaiting a build</span>
      </div>
    </dl>
    <section v-if="!apps.length" class="dashboard-panel dashboard-welcome">
      <span class="empty-app-icon"><UiIcon name="repo" /></span>
      <h2>Connect your first application</h2>
      <p>Choose a repository and configure its build. Your deployments will appear here.</p>
      <UiButton class="primary" @click="$emit('create')"
        ><UiIcon name="plus" />New application</UiButton
      >
    </section>
    <div v-else class="dashboard-grid">
      <section class="dashboard-panel">
        <header class="dashboard-panel-heading">
          <div>
            <p class="panel-eyebrow">RELEASES</p>
            <h2>Recent deployments</h2>
          </div>
          <RouterLink to="/console/deployments">View all <UiIcon name="arrow" /></RouterLink>
        </header>
        <ol v-if="recent.length" class="dashboard-releases">
          <li v-for="deployment in recent" :key="deployment.id">
            <span class="timeline-marker" :data-status="deployment.status"
              ><UiIcon :name="deployment.status === 'live' ? 'check' : 'activity'"
            /></span>
            <div>
              <RouterLink class="release-link" to="/console/deployments"
                ><strong>{{ name(deployment.appId) }}</strong></RouterLink
              >
              <p>
                <code>{{ deployment.commitSha.slice(0, 7) }}</code
                ><span>{{
                  new Date(deployment.createdAt * 1000).toLocaleString([], {
                    month: 'short',
                    day: 'numeric',
                    hour: '2-digit',
                    minute: '2-digit',
                  })
                }}</span>
              </p>
            </div>
            <span class="deployment-status" :data-status="deployment.status">{{
              label(deployment.status)
            }}</span>
          </li>
        </ol>
        <div v-else class="dashboard-quiet">
          <UiIcon name="activity" />
          <h3>No deployments yet</h3>
          <p>Merge an application's workflow PR to start its first build.</p>
        </div>
      </section>
      <section class="dashboard-panel attention-panel">
        <header class="dashboard-panel-heading">
          <div>
            <p class="panel-eyebrow">FOLLOW UP</p>
            <h2>Needs attention</h2>
          </div>
          <UiIcon name="attention" />
        </header>
        <ul v-if="attention.length" class="dashboard-attention">
          <li v-for="app in attention.slice(0, 5)" :key="app.id">
            <UiButton @click="$emit('details', app)"
              ><span
                ><strong>{{ app.domain }}</strong
                ><small>{{
                  latest(app.id)?.status === 'failed'
                    ? 'Latest deployment failed'
                    : 'Awaiting first deployment'
                }}</small></span
              ><UiIcon name="arrow"
            /></UiButton>
          </li>
        </ul>
        <div v-else class="dashboard-quiet">
          <UiIcon name="check" />
          <h3>Nothing needs your attention</h3>
          <p>No failed or unbuilt applications.</p>
        </div>
        <RouterLink v-if="attention.length > 5" class="dashboard-more" to="/console/applications"
          >View all applications <UiIcon name="arrow"
        /></RouterLink>
      </section>
    </div>
  </div>
</template>
