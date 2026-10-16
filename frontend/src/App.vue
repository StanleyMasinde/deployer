<script setup lang="ts">
import { ConfigProvider } from 'reka-ui'
import { ref, onMounted } from 'vue'
import UiIcon from './components/ui/UiIcon.vue'
import { useRoute } from 'vue-router'
const route = useRoute()
const scrollBody = ref<boolean | { padding: number; margin: number }>(true)
onMounted(() => {
  if (CSS.supports('scrollbar-gutter: stable')) scrollBody.value = { padding: 0, margin: 0 }
})
</script>

<template>
  <ConfigProvider :scroll-body="scrollBody">
    <a class="skip-link" href="#main">Skip to content</a>
    <header v-if="!['/console', '/setup'].includes(route.path)" class="masthead">
      <RouterLink class="wordmark" to="/" aria-label="Deployer home">
        <span class="mark" aria-hidden="true">↥</span> Deployer
      </RouterLink>
      <nav aria-label="Main navigation">
        <RouterLink to="/console">Console <UiIcon name="arrow" /></RouterLink>
      </nav>
    </header>
    <RouterView />
    <footer v-if="!['/console', '/setup'].includes(route.path)" class="footer">
      <span>Your server. Your apps.</span><span>Static apps · GitHub Actions · Nginx</span>
    </footer>
  </ConfigProvider>
</template>
