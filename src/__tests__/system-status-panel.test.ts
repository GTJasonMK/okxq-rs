import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import SystemStatusPanel from '@/components/settings/SystemStatusPanel.vue'

describe('系统状态面板', () => {
  it('按后端 snake_case 状态显示配置状态', () => {
    const wrapper = mount(SystemStatusPanel, {
      props: {
        status: {
          okx: {
            api_configured: false,
            demo_configured: false,
            live_configured: true,
            mode: 'live',
          },
          system: {},
          data: {},
        },
        health: null,
      },
    })

    const rows = wrapper.findAll('.ss-row').map(row => row.text())

    expect(rows.some(row => row.includes('当前模式配置') && row.includes('未配置'))).toBe(true)
    expect(rows.some(row => row.includes('模拟盘配置') && row.includes('未配置'))).toBe(true)
    expect(rows.some(row => row.includes('实盘配置') && row.includes('已配置'))).toBe(true)
    expect(rows).toContain('交易模式实盘模式')
  })
})
