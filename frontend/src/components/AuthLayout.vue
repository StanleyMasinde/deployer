<script setup lang="ts">
import "../console.css";
defineProps<{ title: string; description: string }>();
</script>

<template>
  <div class="deployment-console auth-shell">
    <header class="console-sidebar auth-bar">
      <RouterLink class="console-brand" to="/" aria-label="Deployer home">
        <span class="brand-symbol" aria-hidden="true">↥</span>Deployer
      </RouterLink>
      <span class="workspace-label">Your instance</span>
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
