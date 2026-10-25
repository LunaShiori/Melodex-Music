<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import AccountLogin from './AccountLogin.vue'
import Icon from './Icon.vue'
import { ALL_SOURCES, SOURCE_META, type SourceId } from '@/lib/types'
import { useAccountsStore } from '@/stores/accounts'

const accounts = useAccountsStore()

const loginSource = ref<SourceId | null>(null)
const pinEditing = ref(false)
const pinValue = ref('')
const pinConfirm = ref('')
const pinError = ref('')
const unlocking = ref(false)
const unlockPin = ref('')

const minPinLen = computed(() => accounts.status?.minPinLen ?? 6)
const pinProtected = computed(() => Boolean(accounts.status?.pinProtected))
const unlocked = computed(() => Boolean(accounts.status?.unlocked))

onMounted(() => void accounts.refresh())

function stateOf(source: SourceId) {
  const account = accounts.accountOf(source)
  if (account) {
    return { loggedIn: true, text: account.nickname || '已登录', keys: account.keys }
  }
  if (source === 'kuwo') {
    return { loggedIn: false, text: '免登录可用', keys: [] }
  }
  return { loggedIn: false, text: '未登录', keys: [] }
}

async function unlock() {
  pinError.value = ''
  const pin = unlockPin.value.trim()
  if (pin.length < minPinLen.value) {
    pinError.value = `PIN 至少 ${minPinLen.value} 位`
    return
  }
  unlocking.value = true
  try {
    const ok = await accounts.unlock(pin)
    if (ok) {
      unlockPin.value = ''
    } else {
      pinError.value = accounts.error || '解锁失败'
    }
  } finally {
    unlocking.value = false
  }
}

async function savePin() {
  pinError.value = ''
  const pin = pinValue.value.trim()
  if (pin.length < minPinLen.value) {
    pinError.value = `PIN 至少 ${minPinLen.value} 位`
    return
  }
  if (pin !== pinConfirm.value.trim()) {
    pinError.value = '两次输入的 PIN 不一致'
    return
  }
  const ok = await accounts.setPin(pin)
  if (ok) {
    pinEditing.value = false
    pinValue.value = ''
    pinConfirm.value = ''
  } else {
    pinError.value = accounts.error || '设置失败'
  }
}

async function removePin() {
  pinError.value = ''
  await accounts.setPin(null)
  pinEditing.value = false
}

async function logout(source: SourceId) {
  await accounts.clear(source)
}
</script>

<template>
  <section class="card account-panel">
    <div class="panel-head">
      <div>
        <h2>账号</h2>
        <p class="muted">
          未登录时多数歌曲拿不到直链（实测 QQ 音乐几乎全部不可用）。登录只使用你自己账号的权限，
          凭据用 AES-256-GCM 加密后保存在本机，不经过任何服务器。
        </p>
      </div>
      <span class="chip">
        <Icon name="lock" :size="12" />
        {{ pinProtected ? 'PIN 保护已开启' : '本地加密' }}
      </span>
    </div>

    <!-- 凭据库被 PIN 锁住时，先解锁 -->
    <div v-if="accounts.needsUnlock" class="unlock">
      <div class="unlock-tip">
        <Icon name="lock" :size="15" />
        <span>凭据库已启用 PIN 保护，输入 PIN 后才能使用已保存的账号。</span>
      </div>
      <div class="unlock-form">
        <input
          v-model="unlockPin"
          class="input"
          type="password"
          inputmode="numeric"
          autocomplete="off"
          :placeholder="`${minPinLen} 位 PIN`"
          @keyup.enter="unlock"
        />
        <button class="btn btn-primary" type="button" :disabled="unlocking" @click="unlock">
          {{ unlocking ? '解锁中…' : '解锁' }}
        </button>
      </div>
      <p v-if="pinError" class="err">{{ pinError }}</p>
    </div>

    <!-- 已解锁：按音源列账号 -->
    <div v-else class="rows">
      <div v-for="source in ALL_SOURCES" :key="source" class="row">
        <span class="src-dot" :style="{ background: SOURCE_META[source].color }" />
        <div class="who">
          <p class="name">{{ SOURCE_META[source].label }}</p>
          <p class="meta muted">
            <span :class="{ on: stateOf(source).loggedIn }">{{ stateOf(source).text }}</span>
            <template v-if="stateOf(source).keys.length">
              · <span class="mono keys">{{ stateOf(source).keys.join(' / ') }}</span>
            </template>
          </p>
        </div>
        <div class="acts">
          <template v-if="source === 'kuwo'">
            <span class="muted small">无需登录</span>
          </template>
          <template v-else-if="stateOf(source).loggedIn">
            <button class="btn btn-ghost btn-sm" type="button" @click="logout(source)">
              退出登录
            </button>
          </template>
          <template v-else>
            <button class="btn btn-sm" type="button" @click="loginSource = source">
              <Icon :name="source === 'netease' ? 'qr' : 'music'" :size="13" />
              {{ source === 'netease' ? '扫码登录' : '导入 Cookie' }}
            </button>
          </template>
        </div>
      </div>
    </div>

    <!-- PIN 管理 -->
    <div v-if="unlocked" class="pin-row">
      <div class="pin-info">
        <p class="name">PIN 保护</p>
        <p class="meta muted">
          {{
            pinProtected
              ? '已开启：没有 PIN 无法解密凭据库，忘记只能清空重登'
              : '未开启：凭据以随机主密钥加密，主密钥随应用数据目录存放'
          }}
        </p>
      </div>
      <div class="acts">
        <button
          v-if="!pinProtected"
          class="btn btn-sm"
          type="button"
          @click="pinEditing = !pinEditing"
        >
          {{ pinEditing ? '收起' : '设置 PIN' }}
        </button>
        <template v-else>
          <button class="btn btn-ghost btn-sm" type="button" @click="pinEditing = !pinEditing">
            {{ pinEditing ? '收起' : '修改' }}
          </button>
          <button class="btn btn-ghost btn-sm" type="button" @click="removePin">关闭保护</button>
        </template>
      </div>
    </div>

    <div v-if="pinEditing && unlocked" class="pin-form">
      <input
        v-model="pinValue"
        class="input"
        type="password"
        inputmode="numeric"
        autocomplete="new-password"
        :placeholder="`新 PIN（至少 ${minPinLen} 位）`"
      />
      <input
        v-model="pinConfirm"
        class="input"
        type="password"
        inputmode="numeric"
        autocomplete="new-password"
        placeholder="再输入一次"
        @keyup.enter="savePin"
      />
      <button class="btn btn-primary btn-sm" type="button" @click="savePin">保存 PIN</button>
    </div>
    <p v-if="pinEditing && pinError" class="err">{{ pinError }}</p>

    <p class="foot muted tiny">
      提醒：本功能只读取你账号本来就有的权限，不绕过版权与付费限制。请遵守各音源服务条款。
    </p>

    <AccountLogin :source="loginSource" @close="loginSource = null" />
  </section>
</template>

<style scoped>
.account-panel {
  display: grid;
  gap: 12px;
  padding: 4px 16px 14px;
}
.panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.panel-head h2 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}
.panel-head p {
  margin: 5px 0 0;
  font-size: 11.5px;
  line-height: 1.7;
  max-width: 60ch;
}
.rows,
.unlock {
  display: grid;
  gap: 8px;
}
.row,
.pin-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  background: var(--surface-2);
}
.src-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: none;
}
.who,
.pin-info {
  min-width: 0;
  flex: 1;
}
.name {
  margin: 0;
  font-size: 12.5px;
  font-weight: 500;
}
.meta {
  margin: 3px 0 0;
  font-size: 11px;
}
.meta .on {
  color: var(--qq, #23a166);
}
.keys {
  font-family: var(--mono, ui-monospace, monospace);
  font-size: 10.5px;
  opacity: 0.8;
}
.acts {
  display: flex;
  gap: 6px;
  flex: none;
}
.unlock {
  padding: 12px;
  border: 1px dashed var(--line-strong, var(--line));
  border-radius: var(--r-md);
}
.unlock-tip {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-2);
}
.unlock-form {
  display: flex;
  gap: 8px;
}
.unlock-form .input {
  flex: 1;
}
.pin-form {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.pin-form .input {
  flex: 1 1 160px;
}
.foot {
  margin: 2px 0 0;
  line-height: 1.7;
}
.err {
  margin: 0;
  font-size: 11.5px;
  color: var(--netease, #e23b3b);
}
.tiny {
  font-size: 10.5px;
}
.small {
  font-size: 11.5px;
}
@media (max-width: 860px) {
  .row,
  .pin-row {
    padding: 9px 10px;
    gap: 8px;
  }
  .name {
    font-size: 12px;
  }
  .meta {
    font-size: 10.5px;
  }
  /* 按钮留在同一行：让文字区自己收缩，避免整行被撑高 */
  .who,
  .pin-info {
    overflow: hidden;
  }
  .acts {
    flex: none;
  }
}
</style>
