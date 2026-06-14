export type RuntimeSummary = Record<string, unknown>
export type AnyRecord = Record<string, unknown>
export type ParamDraftKind = 'number' | 'boolean' | 'string' | 'json' | 'select'

interface ParamDraftOption {
  label: string
  value: string
}

export interface ReadableParamRow {
  key: string
  label: string
  value: string
  depth: number
  group: boolean
  multiline: boolean
}

export interface ParamDraftRow extends ReadableParamRow {
  input: string
  kind: ParamDraftKind
  error: string
  options?: ParamDraftOption[]
}

export interface EngineParamSpec {
  key: string
  label: string
  kind: ParamDraftKind
  value: unknown
  options?: ParamDraftOption[]
}

export interface EngineParamSpecSource {
  contractMode?: unknown
  costModel?: AnyRecord
  executionModel?: AnyRecord
  params?: AnyRecord
  runtime?: AnyRecord
}

export type ParsedDraftValue =
  | { ok: true; skip: true }
  | { ok: true; skip: false; value: unknown }
  | { ok: false; error: string }
