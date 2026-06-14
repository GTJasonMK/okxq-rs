import type { ThemeSelectOption } from '@/composables/useThemeSelect'

export const RESEARCH_MARKET_TYPE_OPTIONS: ThemeSelectOption[] = [
  { value: 'SPOT', label: '现货' },
  { value: 'SWAP', label: '永续' },
]

export const RESEARCH_TIMEFRAME_OPTIONS: ThemeSelectOption[] = [
  { value: '1H', label: '1H' },
  { value: '4H', label: '4H' },
  { value: '1D', label: '1D' },
]
