import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ThemeDateInput from '@/components/shared/ThemeDateInput.vue'
import {
  buildCalendarDays,
  formatDateValue,
  monthStart,
  parseDateValue,
} from '@/utils/themeDateInput'

describe('ThemeDateInput', () => {
  it('打开自定义日期面板并选择日期', async () => {
    const wrapper = mount(ThemeDateInput, {
      props: {
        modelValue: '2026-05-10',
      },
      global: {
        stubs: {
          Teleport: true,
        },
      },
    })

    await wrapper.get('.td-trigger').trigger('click')

    expect(wrapper.find('.td-panel').exists()).toBe(true)
    expect(wrapper.text()).toContain('2026年5月')

    const day = wrapper.findAll('.td-day')
      .find(item => item.text() === '12' && !item.classes().includes('muted'))
    expect(day).toBeTruthy()
    await day?.trigger('click')

    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['2026-05-12'])
  })

  it('支持清空日期', async () => {
    const wrapper = mount(ThemeDateInput, {
      props: {
        modelValue: '2026-05-10',
      },
    })

    await wrapper.get('.td-clear').trigger('click')

    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([''])
  })

  it('严格解析并格式化 yyyy-mm-dd 日期', () => {
    expect(parseDateValue('2026-02-31')).toBeNull()
    expect(parseDateValue('2026-5-01')).toBeNull()
    expect(formatDateValue(new Date(2026, 4, 1))).toBe('2026-05-01')
  })

  it('生成固定 42 格日历并保留选中、今天和禁用状态', () => {
    const days = buildCalendarDays(
      monthStart(new Date(2026, 4, 1)),
      '2026-05-12',
      '2026-05-10',
      value => value < '2026-05-05',
    )

    expect(days).toHaveLength(42)
    expect(days[0].value).toBe('2026-04-27')
    expect(days.find(day => day.value === '2026-05-12')?.selected).toBe(true)
    expect(days.find(day => day.value === '2026-05-10')?.today).toBe(true)
    expect(days.find(day => day.value === '2026-05-04')?.disabled).toBe(true)
  })
})
