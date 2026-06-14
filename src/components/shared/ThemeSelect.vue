<template>
  <div
    ref="rootRef"
    class="theme-select"
    :class="[`size-${size}`, { open, disabled }]"
  >
    <button
      ref="triggerRef"
      type="button"
      class="ts-trigger"
      role="combobox"
      aria-haspopup="listbox"
      :aria-expanded="open"
      :aria-controls="listboxId"
      :aria-activedescendant="activeOptionId"
      :disabled="disabled"
      @click="toggle"
      @keydown="onTriggerKeydown"
    >
      <span class="ts-label" :class="{ placeholder: !displayOption }">
        {{ displayOption?.label || placeholder }}
      </span>
      <span class="ts-chevron" aria-hidden="true" />
    </button>

    <Teleport to="body">
      <div
        v-if="open"
        :id="listboxId"
        ref="menuRef"
        class="ts-menu"
        :style="menuStyle"
        role="listbox"
        tabindex="-1"
        @keydown="onMenuKeydown"
      >
        <div v-if="showSearch" class="ts-search">
          <input
            ref="searchInputRef"
            v-model="searchText"
            class="ts-search-input"
            type="search"
            placeholder="搜索选项"
            autocomplete="off"
          >
        </div>
        <button
          v-for="(option, index) in visibleOptions"
          :id="optionId(index)"
          :key="`${String(option.value)}-${index}`"
          :data-index="index"
          type="button"
          class="ts-option"
          :class="{
            active: index === activeIndex,
            selected: isSelected(option),
          }"
          role="option"
          :aria-selected="isSelected(option)"
          :disabled="option.disabled"
          @click="selectOption(index)"
          @mousemove="setActive(index)"
        >
          <span class="ts-option-label">{{ option.label }}</span>
          <span v-if="isSelected(option)" class="ts-check" aria-hidden="true" />
        </button>
        <div v-if="visibleOptions.length === 0" class="ts-empty">
          {{ options.length === 0 ? '无可选项' : '无匹配项' }}
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useThemeSelect } from '@/composables/useThemeSelect'
import type { ThemeSelectOption, ThemeSelectValue } from '@/composables/useThemeSelect'

const props = withDefaults(defineProps<{
  modelValue?: ThemeSelectValue
  options: ThemeSelectOption[]
  placeholder?: string
  disabled?: boolean
  size?: 'sm' | 'md'
  searchable?: boolean
  searchThreshold?: number
  menuMaxHeight?: number
  minMenuWidth?: number
}>(), {
  modelValue: '',
  placeholder: '请选择',
  disabled: false,
  size: 'md',
  searchable: false,
  searchThreshold: 10,
  menuMaxHeight: 320,
  minMenuWidth: 0,
})

const emit = defineEmits<{
  'update:modelValue': [value: ThemeSelectValue]
}>()

const searchText = ref('')
const searchInputRef = ref<HTMLInputElement | null>(null)
const displayOption = computed(() =>
  props.options.find(option => option.value === props.modelValue) ?? null
)
const showSearch = computed(() =>
  props.searchable || props.options.length >= props.searchThreshold
)
const visibleOptions = computed(() => {
  const query = searchText.value.trim().toLowerCase()
  if (!query) return props.options
  return props.options.filter(option =>
    option.label.toLowerCase().includes(query) ||
    String(option.value).toLowerCase().includes(query)
  )
})
const selectProps = {
  get modelValue() {
    return props.modelValue
  },
  get options() {
    return visibleOptions.value
  },
  get placeholder() {
    return props.placeholder
  },
  get disabled() {
    return props.disabled
  },
  get size() {
    return props.size
  },
  get menuMaxHeight() {
    return props.menuMaxHeight
  },
  get minMenuWidth() {
    return props.minMenuWidth
  },
}

const {
  rootRef,
  triggerRef,
  menuRef,
  open,
  activeIndex,
  menuStyle,
  activeOptionId,
  listboxId,
  optionId,
  isSelected,
  toggle,
  setActive,
  selectOption,
  onTriggerKeydown,
  onMenuKeydown,
} = useThemeSelect(selectProps, emit)

watch(open, (isOpen) => {
  if (!isOpen) {
    searchText.value = ''
    return
  }
  if (showSearch.value) {
    nextTick(() => searchInputRef.value?.focus())
  }
})
</script>

<style scoped>
.theme-select {
  width: 100%;
  min-width: 0;
}

.ts-trigger {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-primary);
  cursor: pointer;
  font-family: inherit;
  min-width: 0;
  outline: none;
  transition: border-color 0.15s, background 0.15s, box-shadow 0.15s;
}

.size-md .ts-trigger {
  min-height: 30px;
  padding: 5px 8px;
  font-size: 12px;
}

.size-sm .ts-trigger {
  min-height: 26px;
  padding: 4px 8px;
  font-size: 12px;
}

.ts-trigger:hover {
  border-color: rgba(255, 255, 255, 0.18);
  background: rgba(255, 255, 255, 0.035);
}

.ts-trigger:focus-visible,
.theme-select.open .ts-trigger {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px rgba(41, 98, 255, 0.16);
}

.ts-trigger:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.ts-label {
  min-width: 0;
  overflow: hidden;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ts-label.placeholder {
  color: var(--color-text-tertiary);
}

.ts-chevron {
  width: 7px;
  height: 7px;
  flex: 0 0 auto;
  border-right: 1px solid currentColor;
  border-bottom: 1px solid currentColor;
  color: var(--color-text-secondary);
  transform: rotate(45deg) translateY(-2px);
  transition: transform 0.15s;
}

.theme-select.open .ts-chevron {
  transform: rotate(225deg) translate(-1px, -1px);
}

.ts-menu {
  position: fixed;
  z-index: 3000;
  box-sizing: border-box;
  padding: 4px;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.38);
}

.ts-search {
  position: sticky;
  top: -4px;
  z-index: 1;
  margin: -4px -4px 4px;
  padding: 6px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-bg-secondary);
}

.ts-search-input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  font-family: inherit;
  font-size: 12px;
  line-height: 1.3;
  outline: none;
  padding: 7px 9px;
}

.ts-search-input:focus {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px rgba(41, 98, 255, 0.16);
}

.ts-search-input::placeholder {
  color: var(--color-text-tertiary);
}

.ts-option {
  display: flex;
  width: 100%;
  min-height: 28px;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 5px 8px;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-family: inherit;
  font-size: 12px;
  outline: none;
  text-align: left;
}

.ts-option:hover,
.ts-option.active {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}

.ts-option.selected {
  background: var(--color-bg-active);
  color: var(--color-accent);
}

.ts-option:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.ts-option-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ts-check {
  width: 7px;
  height: 12px;
  flex: 0 0 auto;
  border-right: 2px solid currentColor;
  border-bottom: 2px solid currentColor;
  transform: rotate(42deg) translateY(-1px);
}

.ts-empty {
  padding: 8px;
  color: var(--color-text-tertiary);
  font-size: 12px;
  text-align: center;
}
</style>
