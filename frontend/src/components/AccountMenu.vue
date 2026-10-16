<script setup lang="ts">
import { computed } from 'vue'
import {
  DropdownMenuRoot,
  DropdownMenuTrigger,
  DropdownMenuPortal,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from 'reka-ui'
import UiButton from './ui/UiButton.vue'
import UiIcon from './ui/UiIcon.vue'
const props = defineProps<{ user?: { name: string; email: string }; busy?: boolean }>()
defineEmits<{ signOut: [] }>()
const initials = computed(
  () =>
    props.user?.name
      .trim()
      .split(/\s+/)
      .slice(0, 2)
      .map((p) => p[0])
      .join('')
      .toUpperCase() || 'O',
)
</script>
<template>
  <DropdownMenuRoot
    ><DropdownMenuTrigger as-child
      ><UiButton class="account-trigger" aria-label="Account menu" :disabled="busy"
        ><span class="account-avatar" aria-hidden="true">{{ initials }}</span
        ><span class="account-name">{{ user?.name ?? 'Instance owner' }}</span
        ><UiIcon name="chevron" /></UiButton></DropdownMenuTrigger
    ><DropdownMenuPortal
      ><DropdownMenuContent class="account-menu" align="end" :side-offset="8"
        ><DropdownMenuLabel class="account-menu-label"
          ><strong>{{ user?.name ?? 'Instance owner' }}</strong
          ><span v-if="user">{{ user.email }}</span></DropdownMenuLabel
        ><DropdownMenuSeparator class="account-menu-rule" /><DropdownMenuItem as-child
          ><RouterLink to="/console/profile"
            ><UiIcon name="dashboard" />Profile</RouterLink
          ></DropdownMenuItem
        ><DropdownMenuItem as-child
          ><RouterLink to="/console/personal"
            ><UiIcon name="settings" />Personal settings</RouterLink
          ></DropdownMenuItem
        ><DropdownMenuSeparator class="account-menu-rule" /><DropdownMenuItem
          :disabled="busy"
          @select="$emit('signOut')"
          ><UiIcon name="logout" />Sign out</DropdownMenuItem
        ></DropdownMenuContent
      ></DropdownMenuPortal
    ></DropdownMenuRoot
  >
</template>
