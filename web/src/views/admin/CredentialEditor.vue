<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">credentials</p>
          <h3>{{ isEdit ? '编辑凭据' : '新增凭据' }}</h3>
          <p class="admin-section-copy">敏感字段会在服务端加密后存入数据库。</p>
        </div>
        <RouterLink class="ghost-button" to="/admin/credentials">返回列表</RouterLink>
      </div>

      <div v-if="loading" class="loading-panel">正在加载凭据...</div>
      <form v-else class="admin-form-grid" @submit.prevent="submit">
        <label>
          <span>凭据名称</span>
          <input v-model.trim="form.name" required />
        </label>
        <label>
          <span>平台</span>
          <select v-model="form.platform">
            <option value="github">github</option>
            <option value="gitee">gitee</option>
            <option value="gitlab">gitlab</option>
            <option value="other">other</option>
          </select>
        </label>
        <label>
          <span>认证类型</span>
          <select v-model="form.auth_type">
            <option value="token">token</option>
            <option value="ssh">ssh</option>
            <option value="basic">basic</option>
          </select>
        </label>
        <label>
          <span>用户名</span>
          <input v-model.trim="form.username" placeholder="token/basic 可选或必填，ssh 默认为 git" />
        </label>

        <label v-if="form.auth_type === 'token'">
          <span>Token</span>
          <input
            v-model.trim="form.token"
            :placeholder="isEdit ? '留空表示保持不变' : '请输入 Personal Access Token'"
          />
        </label>

        <label v-if="form.auth_type === 'basic'">
          <span>密码</span>
          <input
            v-model.trim="form.password"
            type="password"
            :placeholder="isEdit ? '留空表示保持不变' : '请输入密码'"
          />
        </label>

        <label v-if="form.auth_type === 'ssh'" class="admin-form-full">
          <span>SSH 私钥路径</span>
          <input v-model.trim="form.ssh_key_path" placeholder="例如 C:\\Users\\me\\.ssh\\id_ed25519" />
        </label>

        <button class="primary-button" :disabled="saving" type="submit">
          {{ saving ? '提交中...' : isEdit ? '保存修改' : '创建凭据' }}
        </button>
      </form>

      <p class="admin-hint">
        提示：GitHub token 默认用户名会自动使用 `x-access-token`；其他平台默认使用 `oauth2`，如需指定请手动填写用户名。
      </p>
      <p v-if="error" class="form-error">{{ error }}</p>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import * as adminApi from '../../api/admin'
import type { GitAuthType, GitPlatform } from '../../types'

interface CredentialFormState {
  name: string
  platform: GitPlatform
  auth_type: GitAuthType
  username: string
  token: string
  password: string
  ssh_key_path: string
}

const route = useRoute()
const router = useRouter()
const isEdit = computed(() => Boolean(route.params.id))
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const form = reactive<CredentialFormState>({
  name: '',
  platform: 'github',
  auth_type: 'token',
  username: '',
  token: '',
  password: '',
  ssh_key_path: '',
})

watch(
  () => form.auth_type,
  (authType) => {
    if (authType !== 'token') {
      form.token = ''
    }
    if (authType !== 'basic') {
      form.password = ''
    }
    if (authType !== 'ssh') {
      form.ssh_key_path = ''
    }
  },
)

onMounted(async () => {
  if (!isEdit.value) return
  const credentialId = Number(route.params.id)
  if (!Number.isFinite(credentialId)) {
    await router.replace('/admin/credentials')
    return
  }

  loading.value = true
  try {
    const credential = await adminApi.fetchCredential(credentialId)
    form.name = credential.name
    form.platform = credential.platform
    form.auth_type = credential.auth_type
    form.username = credential.username ?? ''
    form.ssh_key_path = credential.ssh_key_path ?? ''
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载凭据失败'
  } finally {
    loading.value = false
  }
})

async function submit() {
  saving.value = true
  error.value = ''
  try {
    const payload = {
      name: form.name,
      platform: form.platform,
      auth_type: form.auth_type,
      username: form.username.trim() || null,
      token: form.auth_type === 'token' ? form.token.trim() || null : null,
      password: form.auth_type === 'basic' ? form.password.trim() || null : null,
      ssh_key_path: form.auth_type === 'ssh' ? form.ssh_key_path.trim() || null : null,
    }
    if (isEdit.value) {
      await adminApi.updateCredential(Number(route.params.id), payload)
    } else {
      await adminApi.createCredential(payload)
    }
    await router.push('/admin/credentials')
  } catch (err) {
    error.value = err instanceof Error ? err.message : '保存凭据失败'
  } finally {
    saving.value = false
  }
}
</script>
