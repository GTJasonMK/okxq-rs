import { invoke, isTauri } from '@tauri-apps/api/core'
import type { LocalApiParam, LocalApiRequest } from '@/types/api'
import { describeError, logger } from '@/utils/logger'
import { requestKey } from './client/requestKey'
import { unwrapResponse } from './client/response'

type ApiRequestOptions = {
  params?: Record<string, LocalApiParam>
  body?: unknown
  dedupe?: boolean
}

const inflightGetRequests = new Map<string, Promise<unknown>>()

async function apiRequest<T>(
  method: LocalApiRequest['method'],
  path: string,
  options?: ApiRequestOptions,
): Promise<T> {
  if (method === 'GET' && options?.dedupe !== false) {
    const key = requestKey(method, path, options)
    const existing = inflightGetRequests.get(key)
    if (existing) return existing as Promise<T>

    const request = invokeApi<T>(method, path, options).finally(() => {
      if (inflightGetRequests.get(key) === request) {
        inflightGetRequests.delete(key)
      }
    })
    inflightGetRequests.set(key, request)
    return request
  }
  return invokeApi<T>(method, path, options)
}

async function invokeApi<T>(
  method: LocalApiRequest['method'],
  path: string,
  options?: ApiRequestOptions,
): Promise<T> {
  const started = performance.now()
  if (!isTauri()) {
    throw new Error(
      '当前页面没有运行在 Tauri 桌面端，无法访问本地后端；请使用 npm run dev 启动完整桌面端，不要只打开 Vite 前端页面',
    )
  }
  logger.debug('request start', {
    scope: 'api',
    method,
    path,
    params: options?.params,
    body: options?.body,
  })
  try {
    const raw = await invoke('local_api_request', {
      req: { method, path, params: options?.params, body: options?.body },
    })
    const data = unwrapResponse<T>(raw)
    logger.debug('request success', {
      scope: 'api',
      method,
      path,
      durationMs: Math.round(performance.now() - started),
    })
    return data
  } catch (error) {
    logger.error('request failed', {
      scope: 'api',
      method,
      path,
      durationMs: Math.round(performance.now() - started),
      error: describeError(error),
      raw: error,
    })
    throw error
  }
}

export function apiGet<T>(
  path: string,
  params?: Record<string, LocalApiParam>,
  options?: { dedupe?: boolean },
): Promise<T> {
  if (options?.dedupe === false) {
    return apiRequest<T>('GET', path, { params, dedupe: false })
  }
  return apiRequest<T>('GET', path, { params })
}

export function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return apiRequest<T>('POST', path, { body })
}

export function apiPostWithParams<T>(
  path: string,
  body?: unknown,
  params?: Record<string, LocalApiParam>,
): Promise<T> {
  return apiRequest<T>('POST', path, { body, params })
}

export function apiPut<T>(path: string, body?: unknown): Promise<T> {
  return apiRequest<T>('PUT', path, { body })
}

export function apiPatch<T>(path: string, body?: unknown): Promise<T> {
  return apiRequest<T>('PATCH', path, { body })
}

export function apiDelete<T>(path: string, params?: Record<string, LocalApiParam>): Promise<T> {
  return apiRequest<T>('DELETE', path, { params })
}
