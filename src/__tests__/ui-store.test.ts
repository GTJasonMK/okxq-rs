import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useUiStore } from '@/stores/uiStore'

describe('UI 偏好持久化', () => {
  beforeEach(() => {
    window.localStorage.clear()
    setActivePinia(createPinia())
  })

  it('读取当前项目侧栏折叠偏好并写入当前项目 key', () => {
    window.localStorage.setItem('okxq.sidebar.collapsed', '1')

    const store = useUiStore()
    expect(store.sidebarCollapsed).toBe(true)

    store.toggleSidebar()
    expect(store.sidebarCollapsed).toBe(false)
    expect(window.localStorage.getItem('okxq.sidebar.collapsed')).toBe('0')
  })

  it('主题偏好会持久化', () => {
    const store = useUiStore()

    store.setTheme('light')

    expect(store.theme).toBe('light')
    expect(window.localStorage.getItem('okxq.theme')).toBe('light')
  })
})
