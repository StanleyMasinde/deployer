<script setup lang="ts">
import { Primitive } from 'reka-ui'
defineOptions({ inheritAttrs: false })
defineProps<{ error?: string }>()
const model = defineModel<string>({ default: '' })
</script>
<template>
  <Primitive as-child
    ><input
      v-bind="$attrs"
      v-model="model"
      :aria-invalid="error ? true : undefined"
      :aria-describedby="
        error ? `${$attrs.id}-error` : ($attrs['aria-describedby'] as string | undefined)
      "
  /></Primitive>
  <p v-if="error" :id="`${$attrs.id}-error`" class="field-error" role="alert">{{ error }}</p>
</template>
