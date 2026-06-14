export type ThemeDateCalendarDay = {
  value: string
  label: string
  inMonth: boolean
  today: boolean
  selected: boolean
  disabled: boolean
}

export function parseDateValue(value?: string) {
  const match = String(value ?? '').match(/^(\d{4})-(\d{2})-(\d{2})$/)
  if (!match) return null
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const date = new Date(year, month - 1, day)
  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day
  ) {
    return null
  }
  return date
}

export function formatDateValue(date: Date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

export function monthStart(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), 1)
}

export function buildCalendarDays(
  month: Date,
  selectedValue: string,
  todayValue: string,
  isDisabled: (value: string) => boolean,
): ThemeDateCalendarDay[] {
  const start = calendarStart(month)
  return Array.from({ length: 42 }, (_, index) => {
    const date = addDays(start, index)
    const value = formatDateValue(date)
    return {
      value,
      label: String(date.getDate()),
      inMonth: date.getMonth() === month.getMonth(),
      today: value === todayValue,
      selected: value === selectedValue,
      disabled: isDisabled(value),
    }
  })
}

function calendarStart(month: Date) {
  const weekday = month.getDay() || 7
  return addDays(month, 1 - weekday)
}

function addDays(date: Date, days: number) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days)
}
