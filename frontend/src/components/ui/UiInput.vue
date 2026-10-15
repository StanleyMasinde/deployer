<script setup lang="ts">
import { Primitive } from "reka-ui";
const model = defineModel<string>({ default: "" });
</script>

<template>
  <Primitive as-child><input v-model="model" /></Primitive>
</template>
