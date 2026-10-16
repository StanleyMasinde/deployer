<script setup lang="ts">
import UiIcon from './UiIcon.vue'
import { CheckboxRoot, CheckboxIndicator } from 'reka-ui'
const model = defineModel<boolean>({ default: false })
</script>

<template>
  <CheckboxRoot
    class="checkbox-control"
    :model-value="model"
    @update:model-value="model = $event === true"
  >
    <CheckboxIndicator aria-hidden="true"><UiIcon name="check" /></CheckboxIndicator>
  </CheckboxRoot>
</template>
