interface SidebarNavItem {
  path: string
  label: string
  icon: string
}

export interface SidebarNavGroup {
  key: string
  label: string
  items: SidebarNavItem[]
}

export const sidebarNavGroups: SidebarNavGroup[] = [
  {
    key: 'overview',
    label: '总览',
    items: [
      { path: '/', label: '仪表盘', icon: '📊' },
    ],
  },
  {
    key: 'data',
    label: '数据',
    items: [
      { path: '/market', label: '行情', icon: '📈' },
      { path: '/data-center', label: '数据中心', icon: '🗄️' },
    ],
  },
  {
    key: 'trading',
    label: '交易',
    items: [
      { path: '/trading', label: '交易', icon: '💹' },
      { path: '/live-strategy', label: '实盘策略', icon: '🎯' },
      { path: '/backtest', label: '回测', icon: '⚡' },
      { path: '/risk', label: '风险', icon: '🛡️' },
    ],
  },
  {
    key: 'research',
    label: '研究',
    items: [
      { path: '/scanner', label: '扫描', icon: '🔍' },
      { path: '/research', label: '研究', icon: '🧪' },
      { path: '/trend-research', label: '趋势研究', icon: '🔬' },
    ],
  },
  {
    key: 'tools',
    label: '工具',
    items: [
      { path: '/journal', label: '日志', icon: '📝' },
      { path: '/assistant', label: 'AI', icon: '🤖' },
    ],
  },
  {
    key: 'system',
    label: '系统',
    items: [
      { path: '/settings', label: '设置', icon: '⚙️' },
    ],
  },
]
