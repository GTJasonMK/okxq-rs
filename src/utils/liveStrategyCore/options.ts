export function addSymbolOption(values: Set<string>, value: string | undefined) {
  const normalized = value?.trim()
  if (normalized) values.add(normalized)
}

export function withCurrentOption<T extends { value: string; label: string }>(
  options: T[],
  currentValue: string | undefined,
  currentLabel?: string,
): T[] {
  const value = currentValue?.trim()
  if (!value || options.some(option => option.value === value)) return options
  return [
    {
      value,
      label: `${currentLabel?.trim() || value}（当前运行）`,
    } as T,
    ...options,
  ]
}
