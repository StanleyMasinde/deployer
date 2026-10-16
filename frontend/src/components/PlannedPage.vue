<script setup lang="ts">
import { computed } from 'vue'
import UiIcon from './ui/UiIcon.vue'
const props = defineProps<{ area: string }>()
const pages = {
  personal: {
    icon: 'settings',
    heading: 'Your account. Your preferences.',
    description:
      'Personal settings for the instance owner. Theme currently follows your system preference automatically.',
    features: [
      ['Account security', 'Manage your password and review account access.'],
      [
        'Personal preferences',
        'Control optional account preferences while respecting system accessibility settings.',
      ],
      ['Profile', 'Keep your account details up to date.'],
    ],
  },

  runtimes: {
    icon: 'branch',
    heading: 'The tools your applications need.',
    description:
      'Manage application runtimes and package managers on your VPS. Deployer itself remains a self-contained native binary.',
    features: [
      [
        'Installed versions',
        'Inspect Node.js, PHP, pnpm, Bun, and other supported application dependencies.',
      ],
      [
        'Version management',
        'Install supported versions deliberately and select the version an application uses.',
      ],
      [
        'Application requirements',
        'See which applications depend on each runtime before changing or removing it.',
      ],
    ],
  },
  resources: {
    icon: 'resources',
    heading: 'Know what your VPS is carrying.',
    description: 'See server capacity and resource consumption without opening an SSH session.',
    features: [
      ['Server overview', 'Inspect CPU usage, memory and swap, load averages, and disk capacity.'],
      ['Service resource usage', 'See which registered systemd services consume CPU and memory.'],
      ['Recent history', 'Track usage over time and spot sustained resource pressure.'],
    ],
  },
  services: {
    icon: 'server',
    heading: 'Your services. Under your control.',
    description: 'Manage the systemd services already running on your VPS, from the browser.',
    features: [
      ['Service controls', 'Register a unit, inspect its state, and start, stop, or restart it.'],
      ['Journal logs', 'Read and follow service logs without opening an SSH session.'],
      [
        'Release handover',
        'Start a candidate release, verify readiness, switch Nginx traffic, and drain the previous process.',
      ],
    ],
  },
  previews: {
    icon: 'previews',
    heading: 'A place to review every change.',
    description:
      'Isolated, temporary deployments for pull requests, cleaned up when the PR closes.',
    features: [
      ['Per-PR addresses', 'Give each pull request an address such as pr-17.appname.example.com.'],
      [
        'Static and systemd previews',
        'Publish static artifacts or run a separate, resource-limited service.',
      ],
      [
        'Bounded resource use',
        'Limit active previews, expire unused environments, and keep preview secrets separate.',
      ],
    ],
  },
  domains: {
    icon: 'domains',
    heading: 'An address from the first deployment.',
    description:
      'Set an installation domain once, then manage generated addresses and custom domains here.',
    features: [
      [
        'Generated addresses',
        'Assign appname.example.com automatically when an application is created.',
      ],
      ['Custom domains', 'Verify CNAME records and attach additional hostnames to an application.'],
      [
        'Certificate overview',
        'See HTTPS coverage and renewal status across the instance. Existing certificate controls remain in application details.',
      ],
    ],
  },
  settings: {
    icon: 'settings',
    heading: 'Set it up. Let it run.',
    description: 'Keep instance configuration deliberate, visible, and easy to manage.',
    features: [
      [
        'Guided GitHub connection',
        'Create a preconfigured GitHub App through the manifest flow and select repository access.',
      ],
      ['Instance defaults', 'Manage the installation domain and default deployment settings.'],
      [
        'Deployment preferences',
        'Review detected framework settings and keep explicit overrides for your applications.',
      ],
    ],
  },
} as const
const page = computed(() => pages[props.area as keyof typeof pages] ?? pages.services)
</script>
<template>
  <section class="planned-page" aria-labelledby="planned-heading">
    <div class="planned-intro">
      <span class="planned-symbol"><UiIcon :name="page.icon" /></span
      ><span class="coming-soon">Coming soon</span>
      <h2 id="planned-heading">{{ page.heading }}</h2>
      <p>{{ page.description }}</p>
      <p class="planned-note">This page is a preview of planned functionality.</p>
    </div>
    <dl class="planned-features">
      <div v-for="[title, description] in page.features" :key="title">
        <dt>{{ title }}</dt>
        <dd>{{ description }}</dd>
      </div>
    </dl>
  </section>
</template>
