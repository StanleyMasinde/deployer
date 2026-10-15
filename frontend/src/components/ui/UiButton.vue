<script setup lang="ts">
import { Primitive } from "reka-ui";
withDefaults(defineProps<{ type?: "button" | "submit" | "reset" }>(), { type: "button" });
</script>

<template>
  <Primitive as="button" :type="type"><slot /></Primitive>
</template>
