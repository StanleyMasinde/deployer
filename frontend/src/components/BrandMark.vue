<script setup lang="ts">
defineProps<{ size?: number }>()
</script>
<template>
  <svg
    class="brand-mark"
    :width="size ?? 32"
    :height="size ?? 32"
    viewBox="0 0 64 64"
    fill="currentColor"
    aria-hidden="true"
    focusable="false"
  >
    <path d="M6 28V6h22v8H14v14H6Zm30 30v-8h14V36h8v22H36Z" />
    <rect x="22" y="22" width="20" height="20" rx="2" />
  </svg>
</template>
