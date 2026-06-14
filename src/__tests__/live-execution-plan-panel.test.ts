import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import LiveExecutionPlanPanel from '@/components/live/LiveExecutionPlanPanel.vue'
import { liveExecutionPlan } from './fixtures/liveStrategy'

describe('LiveExecutionPlanPanel', () => {
  it('显示计划退出处理中状态，避免暴露英文状态值', () => {
    const wrapper = mount(LiveExecutionPlanPanel, {
      props: {
        plans: [
          liveExecutionPlan({
            status: 'exit_processing',
            updated_at: 1_780_000_100_000,
          }),
        ],
      },
    })

    expect(wrapper.text()).toContain('处理中 1')
    expect(wrapper.text()).toContain('处理中')
    expect(wrapper.text()).toContain('正在处理')
    expect(wrapper.text()).not.toContain('exit_processing')

    wrapper.unmount()
  })
})
