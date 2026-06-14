import { onMounted, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import { describeError, logger } from '@/utils/logger'

const PRICE_ALERT_EVENT = 'okxq-market-alert'

type Unlisten = () => void

export function useGlobalNotifications() {
  let unlistenAlert: Unlisten | null = null

  async function showNotification(payload: unknown) {
    const item = isRecord(payload) ? payload : {}
    const title = stringValue(item.title) || 'OKXQ 价格提醒'
    const body = stringValue(item.message) || stringValue(item.body)

    try {
      if (!(await ensureNotificationPermission())) return
      sendNotification({ title, body })
    } catch (error) {
      logger.warn('desktop notification failed', {
        scope: 'notification',
        error: describeError(error),
      })
    }
  }

  onMounted(async () => {
    try {
      unlistenAlert = await listen(PRICE_ALERT_EVENT, (event) => {
        void showNotification(event.payload)
      })
    } catch (error) {
      logger.warn('price alert listener failed', {
        scope: 'notification',
        error: describeError(error),
      })
    }
  })

  onUnmounted(() => {
    unlistenAlert?.()
    unlistenAlert = null
  })
}

async function ensureNotificationPermission(): Promise<boolean> {
  if (await isPermissionGranted()) return true
  return (await requestPermission()) === 'granted'
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}
