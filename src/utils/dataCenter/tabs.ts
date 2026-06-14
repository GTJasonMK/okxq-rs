import type { DataCenterTab, DataCenterTabItem } from '@/types/dataCenter'

export const DATA_CENTER_TAB_KEY = 'okxq.data-center.active-tab'
export const DEFAULT_DATA_CENTER_TAB: DataCenterTab = 'watchlist'

export const DATA_CENTER_TABS: DataCenterTabItem[] = [
  { key: 'watchlist', label: '数据标的', description: '显示数据库已有标的，并维护已接管采集规则' },
  { key: 'collection', label: '秒级采集', description: '观察实时成交采集器和 1 秒特征柱写入状态' },
  { key: 'inventory', label: '数据库库存', description: '审查本地数据覆盖、表计数和删除残留状态' },
  { key: 'guardian', label: '数据守护', description: '查看后台守护器策略、队列和最近扫描结果' },
]

const VALID_DATA_CENTER_TABS: DataCenterTab[] = ['watchlist', 'collection', 'inventory', 'guardian']

export function normalizeDataCenterTab(value: unknown): DataCenterTab | '' {
  const tab = String(value || '').trim().toLowerCase()
  return VALID_DATA_CENTER_TABS.includes(tab as DataCenterTab) ? tab as DataCenterTab : ''
}

export function dataCenterTabDescription(tabs: DataCenterTabItem[], activeTab: DataCenterTab) {
  return tabs.find(tab => tab.key === activeTab)?.description ?? ''
}
