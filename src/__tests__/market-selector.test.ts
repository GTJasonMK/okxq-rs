import { describe, expect, it } from 'vitest'
import { defineComponent, h, type PropType } from 'vue'
import { mount } from '@vue/test-utils'
import MarketSelector from '@/components/market/MarketSelector.vue'
import { VALID_MARKET_TIMEFRAMES } from '@/utils/marketView'
import type { Timeframe, WatchedSymbol } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'

describe('MarketSelector', () => {
  it('展示全部有效 K 线周期并能选择 3m', async () => {
    const wrapper = mountSelector({
      timeframe: '1m',
    })

    const buttons = wrapper.findAll('.tf-btn')
    expect(buttons.map(button => button.text())).toEqual(VALID_MARKET_TIMEFRAMES)

    await buttons.find(button => button.text() === '3m')?.trigger('click')

    expect(wrapper.emitted('update:timeframe')?.at(-1)).toEqual(['3m'])
  })

  it('关注币种下拉选择后发出标准 symbol', async () => {
    const wrapper = mountSelector({
      symbol: 'BTC-USDT',
      watchedSymbols: [
        watchedSymbol({ symbol: 'BTC-USDT' }),
        watchedSymbol({ symbol: 'ETH-USDT', sync_spot: false, sync_swap: true }),
      ],
    })

    await wrapper.find('select[data-role="watch-symbol"]').setValue('ETH-USDT')

    expect(wrapper.emitted('update:symbol')?.at(-1)).toEqual(['ETH-USDT'])
  })

  it('范围下拉只接受当前周期允许的范围', async () => {
    const wrapper = mountSelector({
      timeframe: '1m',
      rangeDays: 30,
    })

    const rangeSelect = wrapper.find('select[data-role="range"]')
    expect(rangeSelect.findAll('option').map(option => option.attributes('value'))).toEqual([
      '1',
      '3',
      '7',
      '14',
      '30',
    ])

    await rangeSelect.setValue('14')

    expect(wrapper.emitted('update:range-days')?.at(-1)).toEqual([14])
  })
})

function mountSelector(overrides: {
  symbol?: string
  timeframe?: Timeframe
  rangeDays?: CandleRangeDays
  watchedSymbols?: WatchedSymbol[]
} = {}) {
  return mount(MarketSelector, {
    props: {
      symbol: 'BTC-USDT',
      timeframe: '15m',
      rangeDays: 7,
      watchedSymbols: [watchedSymbol()],
      ...overrides,
    },
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
    const role = props.placeholder === '选择关注币种' ? 'watch-symbol' : 'range'
    return () => h('select', {
      'data-role': role,
      value: props.modelValue,
      onChange: (event: Event) => {
        emit('update:modelValue', (event.target as HTMLSelectElement).value)
      },
    }, props.options.map(option => h('option', { value: option.value }, option.label)))
  },
})

function watchedSymbol(overrides: Partial<WatchedSymbol> = {}): WatchedSymbol {
  return {
    symbol: 'BTC-USDT',
    base_ccy: 'BTC',
    spot_inst_id: 'BTC-USDT',
    swap_inst_id: 'BTC-USDT-SWAP',
    sync_spot: true,
    sync_swap: true,
    ...overrides,
  }
}
