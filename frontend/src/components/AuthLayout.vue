<script setup lang="ts">
import BrandMark from './BrandMark.vue'
import '../console.css'
defineProps<{ title: string; description: string }>()
</script>

<template>
  <div class="deployment-console auth-shell">
    <header class="console-sidebar auth-bar">
      <RouterLink class="console-brand" to="/" aria-label="Deployer home">
        <BrandMark />Deployer
      </RouterLink>
    </header>
    <main id="main" class="auth-main">
      <section class="auth-panel" aria-labelledby="auth-title">
        <header class="auth-heading">
          <h1 id="auth-title">{{ title }}</h1>
          <p>{{ description }}</p>
        </header>
        <slot />
      </section>
    </main>
  </div>
</template>
