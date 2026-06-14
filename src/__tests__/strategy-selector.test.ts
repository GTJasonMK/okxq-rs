import { describe, expect, it } from 'vitest'
import { defineComponent, h, nextTick, type PropType } from 'vue'
import { mount } from '@vue/test-utils'
import StrategySelector from '@/components/backtest/StrategySelector.vue'
import type { StrategyMeta } from '@/types'

describe('StrategySelector', () => {
  it('只暴露策略选择，不再把策略参数作为 UI 可编辑面', async () => {
    const wrapper = mountSelector({
      strategies: [
        strategyMeta({
          id: 'strategy_a',
          description: 'Self-contained strategy',
          runtime: {
            symbol: 'BTC-USDT-SWAP',
            inst_type: 'SWAP',
            timeframe: '15m',
            initial_capital: 1000,
            position_size: 0.15,
            stop_loss: 0.03,
            take_profit: 0.06,
          },
        }),
      ],
      modelValue: 'strategy_a',
    })
    await nextTick()

    expect(wrapper.find('input[type="number"]').exists()).toBe(false)
    expect(wrapper.find('input[type="checkbox"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('Self-contained strategy')
    expect(wrapper.text()).toContain('默认运行：BTC-USDT-SWAP · 15m · SWAP')
    expect(wrapper.emitted('update:params')).toBeUndefined()
  })

  it('父级切换策略时只发出策略 id', async () => {
    const wrapper = mountSelector({
      strategies: [
        strategyMeta({ id: 'strategy_a' }),
        strategyMeta({ id: 'strategy_b', name: 'Strategy B' }),
      ],
      modelValue: 'strategy_a',
    })

    await wrapper.setProps({ modelValue: 'strategy_b' })
    await nextTick()

    expect(lastStrategyId(wrapper)).toBe('strategy_b')
    expect(wrapper.emitted('update:params')).toBeUndefined()
  })
})

function mountSelector(props: { strategies: StrategyMeta[]; modelValue?: string }) {
  return mount(StrategySelector, {
    props,
    global: {
      stubs: {
        ThemeSelect: ThemeSelectStub,
      },
    },
  })
}

const ThemeSelectStub = defineComponent({
  name: 'ThemeSelect',
  props: {
    modelValue: {
      type: String,
      default: '',
    },
    options: {
      type: Array as PropType<Array<{ value: string; label: string }>>,
      required: true,
    },
    placeholder: {
      type: String,
      default: '',
    },
    size: {
      type: String,
      default: 'md',
    },
  },
  emits: ['update:modelValue'],
  setup(props, { emit }) {
    return () => h('select', {
      class: 'theme-select-stub',
      'data-size': props.size,
      value: props.modelValue,
      onChange: (event: Event) => {
        emit('update:modelValue', (event.target as HTMLSelectElement).value)
      },
    }, props.options.map(option => h('option', { value: option.value }, option.label)))
  },
})

function strategyMeta(overrides: Partial<StrategyMeta> = {}): StrategyMeta {
  return {
    id: 'strategy_a',
    name: 'Strategy A',
    description: '',
    ...overrides,
  }
}

function lastStrategyId(wrapper: ReturnType<typeof mountSelector>) {
  const emitted = wrapper.emitted('update:strategy-id') ?? []
  return emitted.at(-1)?.[0]
}
