import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'

describe('ThemeSelect', () => {
  it('支持搜索过滤并选择选项', async () => {
    const wrapper = mount(ThemeSelect, {
      props: {
        modelValue: '',
        searchable: true,
        options: [
          { value: 'BTC-USDT-SWAP', label: 'BTC-USDT-SWAP' },
          { value: 'ETH-USDT-SWAP', label: 'ETH-USDT-SWAP' },
          { value: 'SOL-USDT-SWAP', label: 'SOL-USDT-SWAP' },
        ],
      },
      global: {
        stubs: {
          Teleport: true,
        },
      },
    })

    await wrapper.get('.ts-trigger').trigger('click')

    expect(wrapper.find('.ts-search-input').exists()).toBe(true)
    await wrapper.get('.ts-search-input').setValue('eth')

    const options = wrapper.findAll('.ts-option')
    expect(options).toHaveLength(1)
    expect(options[0].text()).toContain('ETH-USDT-SWAP')

    await options[0].trigger('click')

    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['ETH-USDT-SWAP'])
  })
})
