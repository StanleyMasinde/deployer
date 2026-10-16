<script setup lang="ts">
import ConsolePage from '../components/ConsolePage.vue'
definePage({
  path: '/console/deployments/:deploymentId(\\d+)',
  meta: { ssg: false, title: 'Deployment · Deployer' },
})
</script>
<template><ConsolePage /></template>
