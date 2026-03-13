import type { TaskDefinitionStatus, TaskRunStatus, TaskType } from '../types'

export const TASK_TYPE_LABELS: Record<TaskType, string> = {
  git_review: '代码审查',
  test_gen: '测试生成',
  custom: '自定义',
}

export const TASK_DEFINITION_STATUS_LABELS: Record<TaskDefinitionStatus, string> = {
  active: '运行中',
  paused: '已暂停',
}

export const TASK_RUN_STATUS_LABELS: Record<TaskRunStatus, string> = {
  pending: '待执行',
  running: '执行中',
  done: '已完成',
  failed: '失败',
  cancelled: '已取消',
}

export function requiresRepo(taskType: TaskType) {
  return taskType !== 'custom'
}

export function describeCron(expr?: string | null) {
  if (!expr) return '单次任务'
  const trimmed = expr.trim()
  const parts = trimmed.split(/\s+/)
  if (parts.length !== 5) return trimmed

  const [minute, hour, day, month, weekDay] = parts
  if (minute === '0' && hour === '9' && day === '*' && month === '*' && weekDay === '1-5') {
    return '工作日每天 09:00'
  }
  if (minute === '0' && hour.startsWith('*/') && day === '*' && month === '*' && weekDay === '*') {
    return `每 ${hour.slice(2)} 小时一次`
  }
  if (minute.startsWith('*/') && hour === '*' && day === '*' && month === '*' && weekDay === '*') {
    return `每 ${minute.slice(2)} 分钟一次`
  }
  if (day === '*' && month === '*' && weekDay === '*') {
    return `每天 ${hour}:${minute.padStart(2, '0')}`
  }
  return trimmed
}
