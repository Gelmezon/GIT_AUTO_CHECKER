const dateTimeFormatter = new Intl.DateTimeFormat('zh-CN', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
  hour12: false,
})

export function formatDateTime(value?: string | null, fallback = '-') {
  if (!value) return fallback
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return fallback
  return dateTimeFormatter.format(date)
}

export function formatRelativeTime(value?: string | null, fallback = '-') {
  if (!value) return fallback
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return fallback

  const diff = date.getTime() - Date.now()
  const abs = Math.abs(diff)
  const minute = 60_000
  const hour = 60 * minute
  const day = 24 * hour

  if (abs < minute) return diff >= 0 ? '即将开始' : '刚刚'
  if (abs < hour) {
    const minutes = Math.round(abs / minute)
    return diff >= 0 ? `${minutes} 分钟后` : `${minutes} 分钟前`
  }
  if (abs < day) {
    const hours = Math.round(abs / hour)
    return diff >= 0 ? `${hours} 小时后` : `${hours} 小时前`
  }

  const days = Math.round(abs / day)
  return diff >= 0 ? `${days} 天后` : `${days} 天前`
}

export function formatDuration(start?: string | null, end?: string | null) {
  if (!start || !end) return '-'
  const startedAt = new Date(start).getTime()
  const finishedAt = new Date(end).getTime()
  if (Number.isNaN(startedAt) || Number.isNaN(finishedAt)) return '-'

  const totalSeconds = Math.max(0, Math.round((finishedAt - startedAt) / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60

  if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`
  if (minutes > 0) return `${minutes}m ${seconds}s`
  return `${seconds}s`
}

export function toDateTimeLocalValue(value?: string | null) {
  if (!value) return ''
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  const offset = date.getTimezoneOffset()
  const local = new Date(date.getTime() - offset * 60_000)
  return local.toISOString().slice(0, 16)
}

export function dateTimeLocalToIso(value: string) {
  if (!value.trim()) return null
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return null
  return date.toISOString()
}
