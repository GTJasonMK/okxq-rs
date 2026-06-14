export type LocalApiParam = string | number | boolean | string[] | number[] | boolean[]

export interface LocalApiRequest {
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  path: string
  params?: Record<string, LocalApiParam>
  body?: unknown
}

export class ApiError extends Error {
  constructor(
    message: string,
    public code: number,
    public raw?: unknown,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}
