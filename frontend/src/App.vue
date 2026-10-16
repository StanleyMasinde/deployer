<script setup lang="ts">
import { startLiveUpdates, stopLiveUpdates } from './live'
import NavigationProgress from './components/NavigationProgress.vue'
import BrandMark from './components/BrandMark.vue'
import { ConfigProvider } from 'reka-ui'
import { ref, onMounted, watch, onUnmounted } from 'vue'
import UiIcon from './components/ui/UiIcon.vue'
import { useRoute } from 'vue-router'
const route = useRoute()
watch(
  () => route.meta.consoleData?.authenticated,
  (authenticated) => {
    if (authenticated) startLiveUpdates()
    else stopLiveUpdates()
  },
  { immediate: true },
)
onUnmounted(stopLiveUpdates)
const scrollBody = ref<boolean | { padding: number; margin: number }>(true)
onMounted(() => {
  if (CSS.supports('scrollbar-gutter: stable')) scrollBody.value = { padding: 0, margin: 0 }
})
</script>

<template>
  <ConfigProvider :scroll-body="scrollBody">
    <NavigationProgress />
    <a class="skip-link" href="#main">Skip to content</a>
    <header v-if="!route.path.startsWith('/console') && route.path !== '/setup'" class="masthead">
      <RouterLink class="wordmark" to="/" aria-label="Deployer home">
        <BrandMark /> Deployer
      </RouterLink>
      <nav aria-label="Main navigation">
        <RouterLink to="/console">Console <UiIcon name="arrow" /></RouterLink>
      </nav>
    </header>
    <RouterView />
    <footer v-if="!route.path.startsWith('/console') && route.path !== '/setup'" class="footer">
      <span>Your server. Your apps.</span><span>Static apps · GitHub Actions · Nginx</span>
    </footer>
  </ConfigProvider>
</template>
