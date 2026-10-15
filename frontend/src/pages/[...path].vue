<script setup lang="ts">
definePage({ meta: { ssg: false, title: "Page not found · Deployer" } });
</script>

<template>
  <main id="main" class="console">
    <p class="eyebrow">404</p>
    <h1>Page not found.</h1>
    <RouterLink to="/console">Return to your apps →</RouterLink>
  </main>
</template>
