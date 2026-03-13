<template>
  <section class="admin-page">
    <div class="admin-page-header">
      <div>
        <p class="eyebrow">dashboard</p>
        <h3>系统概览</h3>
      </div>
      <button class="ghost-button" type="button" :disabled="loading" @click="load">
        刷新
      </button>
    </div>

    <p v-if="error" class="form-error">{{ error }}</p>
    <div v-if="loading && !dashboard" class="loading-panel">正在加载仪表盘...</div>

    <template v-else-if="dashboard">
      <section class="stat-grid">
        <article class="stat-card">
          <span>仓库总数</span>
          <strong>{{ dashboard.repo_count }}</strong>
        </article>
        <article class="stat-card">
          <span>任务总数</span>
          <strong>{{ dashboard.task_count }}</strong>
        </article>
        <article class="stat-card">
          <span>用户总数</span>
          <strong>{{ dashboard.user_count }}</strong>
        </article>
        <article class="stat-card">
          <span>今日执行</span>
          <strong>{{ dashboard.today_executed_count }}</strong>
        </article>
      </section>

      <section class="admin-card">
        <div class="admin-page-header compact">
          <div>
            <p class="eyebrow">recent</p>
            <h3>最近任务状态</h3>
          </div>
        </div>

        <div v-if="dashboard.recent_tasks.length === 0">
          <EmptyState
            eyebrow="暂无记录"
            title="还没有执行过任务"
            description="等调度器跑起来之后，最近执行状态会显示在这里。"
          />
        </div>
        <div v-else class="admin-table-wrap">
          <table class="admin-table">
            <thead>
              <tr>
                <th>任务</th>
                <th>类型</th>
                <th>仓库</th>
                <th>状态</th>
                <th>计划时间</th>
                <th>更新时间</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="task in dashboard.recent_tasks" :key="task.id">
                <td>{{ task.name }}</td>
                <td>{{ task.task_type }}</td>
                <td>{{ task.repo_name || '-' }}</td>
                <td>
                  <span class="status-chip" :class="task.status">{{ task.status }}</span>
                </td>
                <td>{{ formatDate(task.scheduled_at) }}</td>
                <td>{{ formatDate(task.updated_at) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </template>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import type { AdminDashboard } from '../../types'

const dashboard = ref<AdminDashboard | null>(null)
const loading = ref(false)
const error = ref('')

onMounted(load)

async function load() {
  loading.value = true
  error.value = ''
  try {
    dashboard.value = await adminApi.fetchDashboard()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载仪表盘失败'
  } finally {
    loading.value = false
  }
}

function formatDate(value: string) {
  return new Date(value).toLocaleString()
}
</script>
