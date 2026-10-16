<script setup lang="ts">
import UiIcon from './UiIcon.vue'
import {
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectIcon,
  SelectPortal,
  SelectContent,
  SelectViewport,
  SelectItem,
  SelectItemText,
  SelectItemIndicator,
} from 'reka-ui'
defineOptions({ inheritAttrs: false })
defineProps<{
  id: string
  error?: string
  options: { value: string; label: string }[]
  placeholder?: string
  disabled?: boolean
  required?: boolean
  name?: string
}>()
const model = defineModel<string>({ default: '' })
</script>

<template>
  <SelectRoot v-model="model" :disabled="disabled" :required="required" :name="name">
    <SelectTrigger
      v-bind="$attrs"
      :id="id"
      class="select-trigger"
      :aria-invalid="error ? true : undefined"
      :aria-describedby="error ? `${id}-error` : $attrs['aria-describedby']"
    >
      <SelectValue :placeholder="placeholder" />
      <SelectIcon aria-hidden="true"><UiIcon name="chevron" /></SelectIcon>
    </SelectTrigger>
    <SelectPortal>
      <SelectContent class="select-content" position="popper" :side-offset="4">
        <SelectViewport class="select-viewport">
          <SelectItem
            v-for="option in options"
            :key="option.value"
            :value="option.value"
            class="select-item"
          >
            <SelectItemText>{{ option.label }}</SelectItemText>
            <SelectItemIndicator aria-hidden="true"><UiIcon name="check" /></SelectItemIndicator>
          </SelectItem>
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
  <p v-if="error" :id="`${id}-error`" class="field-error" role="alert">{{ error }}</p>
</template>
