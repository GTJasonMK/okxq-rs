import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

export type ThemeSelectValue = string

export interface ThemeSelectOption {
  label: string
  value: ThemeSelectValue
  disabled?: boolean
}

export type ThemeSelectProps = {
  modelValue?: ThemeSelectValue
  options: ThemeSelectOption[]
  placeholder?: string
  disabled?: boolean
  size?: 'sm' | 'md'
  menuMaxHeight?: number
  minMenuWidth?: number
}

type ThemeSelectEmit = {
  (event: 'update:modelValue', value: ThemeSelectValue): void
}

let selectUid = 0

export function useThemeSelect(props: ThemeSelectProps, emit: ThemeSelectEmit) {
  const uid = ++selectUid
  const listboxId = `theme-select-listbox-${uid}`
  const rootRef = ref<HTMLElement | null>(null)
  const triggerRef = ref<HTMLButtonElement | null>(null)
  const menuRef = ref<HTMLElement | null>(null)
  const open = ref(false)
  const activeIndex = ref(-1)
  const menuStyle = ref<Record<string, string>>({})

  const selectedOption = computed(() =>
    props.options.find(option => option.value === props.modelValue) ?? null
  )

  const activeOptionId = computed(() =>
    open.value && activeIndex.value >= 0 ? optionId(activeIndex.value) : undefined
  )

  function optionId(index: number) {
    return `theme-select-option-${uid}-${index}`
  }

  function isSelected(option: ThemeSelectOption) {
    return option.value === props.modelValue
  }

  function firstEnabledIndex() {
    return props.options.findIndex(option => !option.disabled)
  }

  function selectedIndex() {
    const index = props.options.findIndex(option => option.value === props.modelValue)
    return index >= 0 && !props.options[index]?.disabled ? index : firstEnabledIndex()
  }

  function toggle() {
    if (props.disabled) return
    if (open.value) closeMenu()
    else openMenu()
  }

  function openMenu() {
    if (props.disabled || open.value) return
    activeIndex.value = selectedIndex()
    open.value = true
    nextTick(() => {
      updateMenuPosition()
      scrollActiveIntoView()
    })
    window.addEventListener('resize', updateMenuPosition)
    window.addEventListener('scroll', updateMenuPosition, true)
    document.addEventListener('pointerdown', onDocumentPointerDown, true)
  }

  function closeMenu() {
    if (!open.value) return
    open.value = false
    window.removeEventListener('resize', updateMenuPosition)
    window.removeEventListener('scroll', updateMenuPosition, true)
    document.removeEventListener('pointerdown', onDocumentPointerDown, true)
  }

  function onDocumentPointerDown(event: PointerEvent) {
    const target = event.target as Node | null
    if (!target) return
    if (rootRef.value?.contains(target) || menuRef.value?.contains(target)) return
    closeMenu()
  }

  function updateMenuPosition() {
    const trigger = triggerRef.value
    if (!trigger) return
    const rect = trigger.getBoundingClientRect()
    const gap = 4
    const viewportHeight = window.innerHeight
    const viewportWidth = window.innerWidth
    const menuMaxHeight = props.menuMaxHeight ?? 320
    const estimatedHeight = Math.min(menuMaxHeight, Math.max(36, props.options.length * 30 + 8))
    const spaceBelow = viewportHeight - rect.bottom
    const spaceAbove = rect.top
    const placeAbove = spaceBelow < estimatedHeight && spaceAbove > spaceBelow
    const maxAvailable = placeAbove ? spaceAbove - 12 : spaceBelow - 12
    const maxHeight = Math.max(96, Math.min(menuMaxHeight, maxAvailable))
    const width = Math.min(
      Math.max(rect.width, props.minMenuWidth ?? 0),
      Math.max(160, viewportWidth - 12),
    )
    const left = Math.min(
      Math.max(6, rect.left),
      Math.max(6, viewportWidth - width - 6),
    )
    const top = placeAbove
      ? Math.max(6, rect.top - Math.min(estimatedHeight, maxHeight) - gap)
      : Math.min(viewportHeight - 6, rect.bottom + gap)

    menuStyle.value = {
      left: `${Math.round(left)}px`,
      top: `${Math.round(top)}px`,
      width: `${Math.round(width)}px`,
      maxHeight: `${Math.round(maxHeight)}px`,
    }
  }

  function moveActive(offset: number) {
    if (props.options.length === 0) {
      activeIndex.value = -1
      return
    }
    let next = activeIndex.value
    for (let i = 0; i < props.options.length; i += 1) {
      next = (next + offset + props.options.length) % props.options.length
      if (!props.options[next]?.disabled) {
        activeIndex.value = next
        nextTick(scrollActiveIntoView)
        return
      }
    }
  }

  function setActive(index: number) {
    if (props.options[index]?.disabled) return
    activeIndex.value = index
  }

  function selectOption(index: number) {
    const option = props.options[index]
    if (!option || option.disabled) return
    emit('update:modelValue', option.value)
    closeMenu()
    nextTick(() => triggerRef.value?.focus())
  }

  function scrollActiveIntoView() {
    if (activeIndex.value < 0) return
    const node = menuRef.value?.querySelector<HTMLElement>(`[data-index="${activeIndex.value}"]`)
    node?.scrollIntoView?.({ block: 'nearest' })
  }

  function onTriggerKeydown(event: KeyboardEvent) {
    if (props.disabled) return
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      if (!open.value) openMenu()
      else moveActive(1)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      if (!open.value) openMenu()
      else moveActive(-1)
      return
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      if (!open.value) openMenu()
      else selectOption(activeIndex.value)
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      closeMenu()
    }
  }

  function onMenuKeydown(event: KeyboardEvent) {
    onTriggerKeydown(event)
  }

  watch(
    () => props.options,
    () => {
      if (!open.value) return
      activeIndex.value = selectedIndex()
      nextTick(() => {
        updateMenuPosition()
        scrollActiveIntoView()
      })
    },
    { deep: true }
  )

  watch(
    () => props.modelValue,
    () => {
      if (!open.value) return
      activeIndex.value = selectedIndex()
    }
  )

  watch(
    () => props.disabled,
    disabled => {
      if (disabled) closeMenu()
    }
  )

  onBeforeUnmount(closeMenu)

  return {
    rootRef,
    triggerRef,
    menuRef,
    open,
    activeIndex,
    menuStyle,
    selectedOption,
    activeOptionId,
    listboxId,
    optionId,
    isSelected,
    toggle,
    setActive,
    selectOption,
    onTriggerKeydown,
    onMenuKeydown,
  }
}
