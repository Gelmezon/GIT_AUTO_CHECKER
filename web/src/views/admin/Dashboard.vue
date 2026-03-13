<template>
  <section class="admin-page">
    <div class="admin-page-header">
      <div>
        <p class="eyebrow">dashboard</p>
        <h3>运行总览</h3>
        <p class="admin-section-copy">统计卡片每 30 秒自动刷新，帮助我们快速观察调度健康度。</p>
      </div>
      <div class="admin-toolbar-actions">
        <RouterLink class="primary-button" to="/admin/tasks/new">新建任务</RouterLink>
        <button class="ghost-button" type="button" :disabled="loading" @click="load">
          刷新
        </button>
      </div>
    </div>

    <p v-if="error" class="form-error">{{ error }}</p>
    <div v-if="loading && !dashboard" class="loading-panel">正在加载仪表盘...</div>

    <template v-else-if="dashboard">
      <section class="stat-grid">
        <article class="stat-card">
          <span>仓库总数</span>
          <strong>{{ dashboard.repo_count }}</strong>
          <small>已纳入同步与审查链路</small>
        </article>
        <article class="stat-card">
          <span>任务定义</span>
          <strong>{{ dashboard.task_count }}</strong>
          <small>包括审查、测试生成与自定义任务</small>
        </article>
        <article class="stat-card">
          <span>接收用户</span>
          <strong>{{ dashboard.user_count }}</strong>
          <small>用于接收执行消息与审核报告</small>
        </article>
        <article class="stat-card accent">
          <span>今日执行</span>
          <strong>{{ dashboard.today_executed_count }}</strong>
          <small>完成或失败的执行实例</small>
        </article>
      </section>

      <section class="admin-card">
        <div class="admin-page-header compact">
          <div>
            <p class="eyebrow">recent runs</p>
            <h3>最近执行</h3>
            <p class="admin-section-copy">最近发生的任务运行会在这里按时间倒序展示。</p>
          </div>
          <RouterLink class="ghost-button" to="/admin/runs">查看全部</RouterLink>
        </div>

        <div v-if="dashboard.recent_runs.length === 0">
          <EmptyState
            eyebrow="暂无记录"
            title="还没有执行历史"
            description="任务开始运行后，最近执行状态会自动出现在这里。"
          />
        </div>
        <div v-else class="admin-table-wrap">
          <table class="admin-table" aria-label="最近执行记录">
            <thead>
              <tr>
                <th>任务</th>
                <th>仓库</th>
                <th>状态</th>
                <th>计划时间</th>
                <th>开始时间</th>
                <th>耗时</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="run in dashboard.recent_runs" :key="run.id">
                <td>
                  <strong>{{ run.task_name }}</strong>
                  <p class="table-subline">{{ run.result || '等待查看详细结果' }}</p>
                </td>
                <td>{{ run.repo_name || '-' }}</td>
                <td>
                  <span class="status-chip" :class="run.status">
                    {{ TASK_RUN_STATUS_LABELS[run.status] }}
                  </span>
                </td>
                <td>{{ formatDateTime(run.scheduled_at) }}</td>
                <td>{{ formatDateTime(run.started_at) }}</td>
                <td>{{ formatDuration(run.started_at, run.finished_at) }}</td>
                <td>
                  <RouterLink class="ghost-button" :to="`/admin/runs/${run.id}`">
                    详情
                  </RouterLink>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </template>
  </section>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import type { AdminDashboard } from '../../types'
import { formatDateTime, formatDuration } from '../../utils/date'
import { TASK_RUN_STATUS_LABELS } from '../../utils/tasks'

const dashboard = ref<AdminDashboard | null>(null)
const loading = ref(false)
const error = ref('')

let refreshTimer: number | undefined

onMounted(() => {
  void load()
  refreshTimer = window.setInterval(() => {
    void load(true)
  }, 30_000)
})

onBeforeUnmount(() => {
  if (refreshTimer) {
    window.clearInterval(refreshTimer)
  }
})

async function load(silent = false) {
  if (!silent) {
    loading.value = true
  }
  error.value = ''
  try {
    dashboard.value = await adminApi.fetchDashboard()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载仪表盘失败'
  } finally {
    loading.value = false
  }
}
</script>
