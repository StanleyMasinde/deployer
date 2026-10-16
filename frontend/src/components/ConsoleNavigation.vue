<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  DialogRoot,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  DialogClose,
} from 'reka-ui'
import UiButton from './ui/UiButton.vue'
import {
  NavigationMenuRoot,
  NavigationMenuList,
  NavigationMenuItem,
  NavigationMenuLink,
} from 'reka-ui'
import UiIcon from './ui/UiIcon.vue'
defineProps<{ active: string; mobile?: boolean }>()
const expanded = ref(false)
const route = useRoute()
watch(
  () => route.path,
  () => {
    expanded.value = false
  },
)
const destinations = [
  { key: 'dashboard', path: '/console', label: 'Dashboard', icon: 'dashboard' },
  { key: 'apps', path: '/console/applications', label: 'Applications', icon: 'apps' },
  { key: 'activity', path: '/console/deployments', label: 'Deployments', icon: 'activity' },
  { key: 'resources', path: '/console/resources', label: 'Resource usage', icon: 'resources' },
  { key: 'services', path: '/console/services', label: 'Services', icon: 'server' },
  { key: 'previews', path: '/console/previews', label: 'PR previews', icon: 'previews' },
  { key: 'domains', path: '/console/domains', label: 'Domains', icon: 'domains' },
  { key: 'settings', path: '/console/settings', label: 'Settings', icon: 'settings' },
  { key: 'runtimes', path: '/console/runtimes', label: 'Runtimes & tools', icon: 'branch' },
] as const
</script>
<template>
  <NavigationMenuRoot
    :class="mobile ? 'mobile-console-nav' : 'desktop-console-nav'"
    :orientation="mobile ? 'horizontal' : 'vertical'"
    :aria-label="mobile ? 'Mobile navigation' : 'Console navigation'"
  >
    <NavigationMenuList>
      <NavigationMenuItem
        v-for="item in mobile ? destinations.slice(0, 3) : destinations"
        :key="item.key"
      >
        <NavigationMenuLink as-child :active="active === item.key">
          <RouterLink :to="item.path" :aria-current="active === item.key ? 'page' : undefined"
            ><UiIcon :name="item.icon" /><span>{{ item.label }}</span></RouterLink
          >
        </NavigationMenuLink>
      </NavigationMenuItem>
      <NavigationMenuItem v-if="mobile"
        ><UiButton
          class="mobile-more"
          aria-haspopup="dialog"
          :aria-expanded="expanded"
          :data-active="!['dashboard', 'apps', 'activity'].includes(active)"
          @click="expanded = true"
          ><UiIcon name="menu" /><span>More</span></UiButton
        ></NavigationMenuItem
      >
    </NavigationMenuList>
  </NavigationMenuRoot>
  <DialogRoot v-if="mobile" v-model:open="expanded"
    ><DialogPortal
      ><DialogOverlay class="console-dialog-overlay" /><DialogContent
        class="console-modal mobile-nav-menu"
        ><header class="mobile-menu-heading">
          <DialogTitle>Workspace</DialogTitle
          ><DialogClose as-child
            ><UiButton aria-label="Close navigation"><UiIcon name="close" /></UiButton
          ></DialogClose>
        </header>
        <DialogDescription>Navigate your Deployer instance.</DialogDescription>
        <nav aria-label="All workspace pages">
          <RouterLink
            v-for="item in destinations"
            :key="item.key"
            :to="item.path"
            :aria-current="active === item.key ? 'page' : undefined"
            @click="expanded = false"
            ><UiIcon :name="item.icon" />{{ item.label
            }}<span
              v-if="
                ['runtimes', 'resources', 'services', 'previews', 'domains', 'settings'].includes(
                  item.key,
                )
              "
              class="planned-nav-label"
              >Soon</span
            ></RouterLink
          >
        </nav></DialogContent
      ></DialogPortal
    ></DialogRoot
  >
</template>
