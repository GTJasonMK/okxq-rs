import type {
  MarginMode,
  OrderSide,
  OrderType,
  PositionSide,
} from '@/types'
import type { ThemeSelectOption } from '@/composables/useThemeSelect'

export type ContractOrderIntent = 'open_long' | 'close_long' | 'open_short' | 'close_short'

export type OrderFormDraft = {
  inst_type: 'SPOT' | 'SWAP'
  inst_id: string
  side: OrderSide
  ord_type: OrderType
  sz: number
  px: number
  td_mode: MarginMode
  lever: number
  pos_side: PositionSide
  reduce_only: boolean
  sync_leverage: boolean
}

export type ContractIntentOption = {
  value: ContractOrderIntent
  label: string
  side: OrderSide
  pos_side: PositionSide
  reduce_only: boolean
}

export type OrderTypeOption = {
  value: OrderType
  label: string
}

export type SideOption = {
  value: OrderSide
  label: string
}

export type SelectOption = ThemeSelectOption
