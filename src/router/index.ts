import { createRouter, createWebHashHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'dashboard',
    component: () => import('@/views/DashboardView.vue'),
    meta: { title: '仪表盘', icon: 'dashboard' },
  },
  {
    path: '/market',
    name: 'market',
    component: () => import('@/views/MarketView.vue'),
    meta: { title: '行情', icon: 'market' },
  },
  {
    path: '/data-center',
    name: 'data-center',
    component: () => import('@/views/DataCenterView.vue'),
    meta: { title: '数据中心', icon: 'data' },
  },
  {
    path: '/trading',
    name: 'trading',
    component: () => import('@/views/TradingView.vue'),
    meta: { title: '交易', icon: 'trading' },
  },
  {
    path: '/backtest',
    name: 'backtest',
    component: () => import('@/views/BacktestView.vue'),
    meta: { title: '回测', icon: 'backtest' },
  },
  {
    path: '/live-strategy',
    name: 'live-strategy',
    component: () => import('@/views/LiveStrategyView.vue'),
    meta: { title: '实盘策略', icon: 'live' },
  },
  {
    path: '/strategy-execution',
    name: 'strategy-execution',
    redirect: '/backtest',
  },
  {
    path: '/scanner',
    name: 'scanner',
    component: () => import('@/views/ScannerView.vue'),
    meta: { title: '扫描', icon: 'scanner' },
  },
  {
    path: '/risk',
    name: 'risk',
    component: () => import('@/views/RiskView.vue'),
    meta: { title: '风险', icon: 'risk' },
  },
  {
    path: '/journal',
    name: 'journal',
    component: () => import('@/views/JournalView.vue'),
    meta: { title: '日志', icon: 'journal' },
  },
  {
    path: '/assistant',
    name: 'assistant',
    component: () => import('@/views/AssistantView.vue'),
    meta: { title: 'AI 助手', icon: 'assistant' },
  },
  {
    path: '/research',
    name: 'research',
    component: () => import('@/views/ResearchView.vue'),
    meta: { title: '研究', icon: 'research' },
  },
  {
    path: '/trend-research',
    name: 'trend-research',
    component: () => import('@/views/TrendResearchView.vue'),
    meta: { title: '趋势研究', icon: 'trend' },
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/views/SettingsView.vue'),
    meta: { title: '设置', icon: 'settings' },
  },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

router.beforeEach((to) => {
  const title = typeof to.meta.title === 'string' ? to.meta.title : '首页'
  document.title = `${title} - OKXQ`
})

export default router
