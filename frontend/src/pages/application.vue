<script setup lang="ts">
import ConsolePage from '../components/ConsolePage.vue'
definePage({
  path: '/console/applications/:appId(\\d+)',
  meta: { ssg: false, title: 'Application · Deployer' },
})
</script>
<template><ConsolePage /></template>
