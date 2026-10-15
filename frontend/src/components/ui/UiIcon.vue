<script setup lang="ts">
defineProps<{
  name:
    | "apps"
    | "activity"
    | "server"
    | "arrow"
    | "plus"
    | "search"
    | "branch"
    | "repo"
    | "close"
    | "logout"
    | "refresh"
    | "check";
}>();
const paths = {
  apps: "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
  activity: "M3 12h4l3-8 4 16 3-8h4",
  server: "M4 3h16v7H4z M4 14h16v7H4z M7 6.5h.01 M7 17.5h.01 M11 6.5h6 M11 17.5h6",
  arrow: "M7 17 17 7 M7 7h10v10",
  plus: "M12 5v14 M5 12h14",
  search: "M21 21l-5-5 M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
  branch:
    "M6 7v10 M18 7v3a4 4 0 0 1-4 4h-4 M9 4a3 3 0 1 1-6 0 3 3 0 0 1 6 0 M9 20a3 3 0 1 1-6 0 3 3 0 0 1 6 0 M21 4a3 3 0 1 1-6 0 3 3 0 0 1 6 0",
  repo: "M5 3h14v18l-7-3-7 3z M9 7h6 M9 11h6",
  close: "m6 6 12 12 M6 18 18 6",
  logout: "M9 4H4v16h5 M9 12h12 M16 7l5 5-5 5",
  refresh: "M20 7v5h-5 M4 17v-5h5 M5.5 7a7 7 0 0 1 11.8-2L20 8 M4 16l2.7 3a7 7 0 0 0 11.8-2",
  check: "m5 12 4 4L19 6",
};
</script>

<template>
  <svg
    class="ui-icon"
    width="18"
    height="18"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[name]" />
  </svg>
</template>
