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
} from "@heroicons/vue/24/outline";

const icons = {
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
};
defineProps<{ name: keyof typeof icons }>();
</script>

<template>
  <component :is="icons[name]" class="ui-icon" width="18" height="18" aria-hidden="true" />
</template>
