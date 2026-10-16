<script setup lang="ts">
import {
  Squares2X2Icon,
  BoltIcon,
  ServerStackIcon,
  ArrowUpRightIcon,
  PlusIcon,
  MagnifyingGlassIcon,
  CodeBracketIcon,
  BookOpenIcon,
  XMarkIcon,
  ArrowRightStartOnRectangleIcon,
  ArrowPathIcon,
  CheckIcon,
  ChevronDownIcon,
  HomeIcon,
  GlobeAltIcon,
  BeakerIcon,
  Cog6ToothIcon,
  Bars3Icon,
  ChartBarIcon,
  ExclamationTriangleIcon,
} from '@heroicons/vue/24/outline'

const icons = {
  dashboard: HomeIcon,
  domains: GlobeAltIcon,
  previews: BeakerIcon,
  settings: Cog6ToothIcon,
  menu: Bars3Icon,
  resources: ChartBarIcon,
  attention: ExclamationTriangleIcon,
  apps: Squares2X2Icon,
  activity: BoltIcon,
  server: ServerStackIcon,
  arrow: ArrowUpRightIcon,
  plus: PlusIcon,
  search: MagnifyingGlassIcon,
  branch: CodeBracketIcon,
  repo: BookOpenIcon,
  close: XMarkIcon,
  logout: ArrowRightStartOnRectangleIcon,
  refresh: ArrowPathIcon,
  check: CheckIcon,
  chevron: ChevronDownIcon,
}
defineProps<{ name: keyof typeof icons }>()
</script>

<template>
  <component :is="icons[name]" class="ui-icon" width="18" height="18" aria-hidden="true" />
</template>
