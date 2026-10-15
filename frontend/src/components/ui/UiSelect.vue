<script setup lang="ts">
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
} from "reka-ui";
defineOptions({ inheritAttrs: false });
defineProps<{
  id: string;
  options: { value: string; label: string }[];
  placeholder?: string;
  disabled?: boolean;
  required?: boolean;
  name?: string;
}>();
const model = defineModel<string>({ default: "" });
</script>

<template>
  <SelectRoot v-model="model" :disabled="disabled" :required="required" :name="name">
    <SelectTrigger v-bind="$attrs" :id="id" class="select-trigger">
      <SelectValue :placeholder="placeholder" />
      <SelectIcon aria-hidden="true">▾</SelectIcon>
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
            <SelectItemIndicator aria-hidden="true">✓</SelectItemIndicator>
          </SelectItem>
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>
