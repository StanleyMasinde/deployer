<script setup lang="ts">
import { navigationPending } from '../router/loading'
</script>
<template>
  <div
    v-if="navigationPending"
    class="navigation-progress"
    role="progressbar"
    aria-label="Loading page"
    aria-valuetext="Loading"
  >
    <span />
  </div>
  <span class="sr-status" role="status" aria-live="polite">{{
    navigationPending ? 'Loading page…' : ''
  }}</span>
</template>
