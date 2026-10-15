<script setup lang="ts">
import { CheckboxRoot, CheckboxIndicator } from "reka-ui";
const model = defineModel<boolean>({ default: false });
</script>

<template>
  <CheckboxRoot
    class="checkbox-control"
    :model-value="model"
    @update:model-value="model = $event === true"
  >
    <CheckboxIndicator aria-hidden="true">✓</CheckboxIndicator>
  </CheckboxRoot>
</template>
