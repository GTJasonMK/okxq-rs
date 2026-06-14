export type EnabledWatchInstType = 'SPOT' | 'SWAP'

export interface EnabledWatchScope {
  symbol: string
  inst_id: string
  inst_type: EnabledWatchInstType
  base_ccy?: string
}
