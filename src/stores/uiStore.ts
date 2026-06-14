import { defineStore } from 'pinia'
import { ref } from 'vue'

const SIDEBAR_COLLAPSED_KEY = 'okxq.sidebar.collapsed'
const THEME_KEY = 'okxq.theme'

type Theme = 'dark' | 'light'

export const useUiStore = defineStore('ui', () => {
  const sidebarCollapsed = ref(readStoredBoolean(SIDEBAR_COLLAPSED_KEY) ?? false)
  const theme = ref<Theme>(readStoredTheme() ?? 'dark')
  const toasts = ref<Array<{ id: number; message: string; type: 'success' | 'error' | 'info' }>>([])
  let toastId = 0

  function setSidebarCollapsed(value: boolean) {
    sidebarCollapsed.value = value
    writeStorage(SIDEBAR_COLLAPSED_KEY, value ? '1' : '0')
  }

  function toggleSidebar() {
    setSidebarCollapsed(!sidebarCollapsed.value)
  }

  function setTheme(t: Theme) {
    theme.value = t
    writeStorage(THEME_KEY, t)
  }

  function addToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
    const id = ++toastId
    toasts.value.push({ id, message, type })
    setTimeout(() => { toasts.value = toasts.value.filter(t => t.id !== id) }, 4000)
  }

  return { sidebarCollapsed, theme, toasts, setSidebarCollapsed, toggleSidebar, setTheme, addToast }
})

function readStoredBoolean(key: string): boolean | null {
  const raw = readStorage(key)
  if (raw === '1' || raw === 'true') return true
  if (raw === '0' || raw === 'false') return false
  return null
}

function readStoredTheme(): Theme | null {
  const raw = readStorage(THEME_KEY)
  return raw === 'dark' || raw === 'light' ? raw : null
}

function readStorage(key: string): string | null {
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

function writeStorage(key: string, value: string) {
  try {
    window.localStorage.setItem(key, value)
  } catch {
    // Local storage can be unavailable in restricted WebView contexts.
  }
}
