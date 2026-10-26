<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import Icon from './Icon.vue'
import { SOURCE_META, type QrStatus, type SourceId } from '@/lib/types'
import { transport } from '@/lib/transport'
import { useAccountsStore } from '@/stores/accounts'

type LoginMode = 'web' | 'qr' | 'cookie'

const props = defineProps<{ source: SourceId | null }>()
const emit = defineEmits<{ close: [] }>()

const accounts = useAccountsStore()

/** 登录方式：桌面默认内置登录窗口，手机与网页预览走扫码 / 粘贴 */
const mode = ref<LoginMode>('cookie')
/** 当前平台是否支持内置登录窗口（Android 的 WebView 读不到 Cookie） */
const webSupported = ref(false)
/** 探测完平台能力之前先不渲染面板，避免标签页闪一下再换 */
const ready = ref(false)
const webBusy = ref(false)
const qrSvg = ref('')
const qrImage = ref('')
const qrKey = ref('')
const qrState = ref<QrStatus | 'loading' | 'error'>('loading')
const qrMessage = ref('')
const remain = ref(0)
const cookieInput = ref('')
const nickname = ref('')
const submitting = ref(false)
const failure = ref('')

let pollTimer: number | null = null
let tickTimer: number | null = null
let stopWeb: (() => void) | null = null

const meta = computed(() => (props.source ? SOURCE_META[props.source] : null))
/** 网易云与 QQ 音乐都有官方扫码接口；酷我免登录，用不上 */
const supportsQr = computed(() => props.source === 'netease' || props.source === 'qq')

const modes = computed(() => {
  const list: { id: LoginMode; label: string; icon: string }[] = []
  if (webSupported.value) list.push({ id: 'web', label: '内置登录', icon: 'external' })
  if (supportsQr.value) list.push({ id: 'qr', label: '扫码登录', icon: 'qr' })
  list.push({ id: 'cookie', label: '粘贴 Cookie', icon: 'music' })
  return list
})

const STATUS_TEXT: Record<string, string> = {
  loading: '正在获取二维码…',
  waiting: '请用手机扫码',
  scanned: '已扫描，请在手机上确认',
  confirmed: '登录成功',
  expired: '二维码已过期',
  error: '获取二维码失败',
}

function stopTimers() {
  if (pollTimer !== null) window.clearInterval(pollTimer)
  if (tickTimer !== null) window.clearInterval(tickTimer)
  pollTimer = null
  tickTimer = null
}

function teardownWeb() {
  stopWeb?.()
  stopWeb = null
}

/** 内置登录窗口：后端读 Cookie、存库、关窗，结果通过事件回推 */
async function openWeb() {
  if (!props.source) return
  failure.value = ''
  webBusy.value = true
  teardownWeb()
  stopWeb = transport.onLoginWeb((event) => {
    if (event.type === 'done') {
      webBusy.value = false
      teardownWeb()
      void accounts.refresh()
      window.setTimeout(() => emit('close'), 600)
    } else if (event.type === 'error') {
      webBusy.value = false
      teardownWeb()
      failure.value = event.message
    } else {
      // 用户自己关掉了窗口
      webBusy.value = false
      teardownWeb()
    }
  })
  const opened = await accounts.startWebLogin(props.source)
  if (!opened) {
    webBusy.value = false
    teardownWeb()
    failure.value = accounts.error || '无法打开登录窗口'
  }
}

async function startQr() {
  if (!props.source) return
  stopTimers()
  qrState.value = 'loading'
  qrMessage.value = ''
  failure.value = ''
  try {
    const ticket = await accounts.startQr(props.source)
    // 网易云给的是 URL，本地渲染成 SVG；QQ 只给 PNG 位图，直接当 data URI 用
    qrSvg.value = ticket.qrSvg ?? ''
    qrImage.value = ticket.qrImage ?? ''
    qrKey.value = ticket.key
    qrState.value = 'waiting'
    remain.value = ticket.expiresIn
    tickTimer = window.setInterval(() => {
      remain.value = Math.max(0, remain.value - 1)
      if (remain.value === 0) {
        stopTimers()
        qrState.value = 'expired'
      }
    }, 1000)
    pollTimer = window.setInterval(() => void pollQr(), 1800)
  } catch (err) {
    qrState.value = 'error'
    failure.value = err instanceof Error ? err.message : String(err)
  }
}

async function pollQr() {
  if (!props.source || !qrKey.value) return
  try {
    const poll = await accounts.pollQr(props.source, qrKey.value)
    qrState.value = poll.status
    qrMessage.value = poll.message ?? ''
    if (poll.status === 'confirmed') {
      stopTimers()
      // 给用户看一眼"登录成功"再关闭
      window.setTimeout(() => emit('close'), 900)
    } else if (poll.status === 'expired' || poll.status === 'error') {
      // 过期与链路出错都要停轮询，否则会一直打后端
      stopTimers()
    }
  } catch (err) {
    stopTimers()
    qrState.value = 'error'
    failure.value = err instanceof Error ? err.message : String(err)
  }
}

async function submitCookie() {
  if (!props.source) return
  failure.value = ''
  const cookie = cookieInput.value.trim()
  if (!cookie.includes('=')) {
    failure.value = '这看起来不是 Cookie，请粘贴「name=value」形式的完整内容'
    return
  }
  submitting.value = true
  try {
    const account = await accounts.saveCookie(props.source, cookie, nickname.value.trim() || null)
    if (account) emit('close')
    else failure.value = accounts.error || '保存失败'
  } finally {
    submitting.value = false
  }
}

async function openHome() {
  if (!meta.value) return
  try {
    await transport.openUrl(meta.value.home)
  } catch {
    /* 没有浏览器可打开时忽略 */
  }
}

watch(
  () => props.source,
  async (source) => {
    stopTimers()
    teardownWeb()
    ready.value = false
    if (!source) return
    cookieInput.value = ''
    nickname.value = ''
    failure.value = ''
    webBusy.value = false
    qrSvg.value = ''
    qrImage.value = ''
    qrKey.value = ''
    qrState.value = 'loading'
    qrMessage.value = ''
    webSupported.value = await transport.loginWebSupported().catch(() => false)
    // 等待期间用户可能又换了音源，别用旧结果覆盖新弹窗
    if (props.source !== source) return
    mode.value = webSupported.value ? 'web' : supportsQr.value ? 'qr' : 'cookie'
    ready.value = true
    if (mode.value === 'qr') void startQr()
  },
  { immediate: true },
)

watch(mode, (next) => {
  if (next === 'qr' && qrState.value !== 'waiting') void startQr()
  if (next !== 'qr') stopTimers()
  if (next !== 'web') {
    webBusy.value = false
    teardownWeb()
  }
})

onBeforeUnmount(() => {
  stopTimers()
  teardownWeb()
})
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="source && meta" class="mask" @click.self="emit('close')">
        <div class="dialog" role="dialog" aria-modal="true">
          <header class="d-head">
            <div class="who">
              <span class="dot" :style="{ background: meta.color }" />
              <div>
                <h3>登录 {{ meta.label }}</h3>
                <p class="muted">
                  只使用你自己账号的权限，凭据加密保存在本机
                </p>
              </div>
            </div>
            <button class="icon-btn" type="button" @click="emit('close')">
              <Icon name="close" :size="16" />
            </button>
          </header>

          <p v-if="!ready" class="muted small preparing">正在准备登录方式…</p>

          <template v-else>
            <div v-if="modes.length > 1" class="tabs">
              <button
                v-for="item in modes"
                :key="item.id"
                class="tab"
                :class="{ on: mode === item.id }"
                type="button"
                @click="mode = item.id"
              >
                <Icon :name="item.icon" :size="14" /> {{ item.label }}
              </button>
            </div>

            <!-- 内置登录窗口：推荐方式，登录一次即可 -->
            <section v-if="mode === 'web'" class="web-wrap">
              <div class="web-side">
                <p class="qr-state" :class="webBusy ? 'waiting' : 'idle'">
                  <span class="bulb" />
                  {{ webBusy ? '已打开登录窗口，等你在里面完成登录' : '在自己的登录页里登录，不用碰 Cookie' }}
                </p>
                <ol class="steps">
                  <li>点下面的按钮，弹出一个 {{ meta.short }} 的登录窗口</li>
                  <li>
                    在窗口里用
                    {{ source === 'qq' ? 'QQ、微信或扫码' : '账号或扫码' }}
                    登录
                  </li>
                  <li>登录成功后窗口自动关闭，凭据加密存到本机</li>
                </ol>
                <button class="btn btn-primary btn-sm web-btn" type="button" :disabled="webBusy" @click="openWeb">
                  <Icon name="external" :size="14" />
                  {{ webBusy ? '等待登录…' : '打开登录窗口' }}
                </button>
                <p v-if="webBusy" class="muted small">
                  窗口没出现？再看一眼任务栏，或在下方改用扫码登录。
                </p>
              </div>
            </section>

            <!-- 扫码 -->
            <section v-else-if="mode === 'qr'" class="qr-wrap">
              <div class="qr-box" :class="qrState">
                <div v-if="qrSvg" class="qr-svg" v-html="qrSvg" />
                <img v-else-if="qrImage" class="qr-img" :src="qrImage" alt="扫码登录二维码" />
                <div v-if="qrState === 'expired' || qrState === 'error'" class="qr-mask">
                  <button class="btn btn-ghost btn-sm" type="button" @click="startQr">
                    <Icon name="refresh" :size="14" /> 重新获取
                  </button>
                </div>
              </div>
              <div class="qr-side">
                <p class="qr-state" :class="qrState">
                  <span class="bulb" />
                  {{ STATUS_TEXT[qrState] ?? qrState }}
                </p>
                <p v-if="qrMessage && qrState !== 'confirmed'" class="muted small">
                  {{ qrMessage }}
                </p>
                <p v-if="qrState === 'waiting' || qrState === 'scanned'" class="muted small">
                  {{
                    props.source === 'qq'
                      ? '用 QQ、微信或 QQ 音乐 App 扫描，并在手机上确认'
                      : '打开网易云音乐 App → 右上角「扫一扫」'
                  }}
                </p>
                <p v-if="remain > 0 && qrState !== 'confirmed'" class="countdown mono">
                  {{ Math.floor(remain / 60) }}:{{ String(remain % 60).padStart(2, '0') }} 后过期
                </p>
                <ol class="steps">
                  <li>手机扫码</li>
                  <li>在手机上点「确认登录」</li>
                  <li>本窗口自动完成登录</li>
                </ol>
              </div>
            </section>

            <!-- Cookie 粘贴 -->
            <section v-else class="cookie-wrap">
              <p class="hint">
                在浏览器打开
                <button class="link" type="button" @click="openHome">{{ meta.label }}</button>
                并登录，然后按 F12 → Network → 任选一个请求 → 复制 Request Headers 里的
                <code>Cookie</code> 整行粘贴到下面。
              </p>
              <textarea
                v-model="cookieInput"
                class="input cookie-input"
                rows="4"
                spellcheck="false"
                placeholder="MUSIC_U=...; __csrf=... 或 uin=...; qm_keystore=..."
              />
              <input
                v-model="nickname"
                class="input"
                type="text"
                :placeholder="source === 'qq' ? '备注名（可留空，自动识别 QQ 号）' : '备注名（可留空）'"
              />
              <p class="muted small">
                只保存在本机加密保险库；不会上传到任何服务器，也不会被写入日志。
              </p>
            </section>
          </template>

          <p v-if="failure" class="err">{{ failure }}</p>

          <footer class="d-foot">
            <span class="muted tiny">
              <Icon name="lock" :size="12" /> AES-256-GCM 本地加密
            </span>
            <div class="acts">
              <button class="btn btn-ghost" type="button" @click="emit('close')">取消</button>
              <button
                v-if="mode === 'cookie'"
                class="btn btn-primary"
                type="button"
                :disabled="submitting || !cookieInput.trim()"
                @click="submitCookie"
              >
                {{ submitting ? '保存中…' : '保存并登录' }}
              </button>
            </div>
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.mask {
  position: fixed;
  inset: 0;
  z-index: 90;
  background: rgba(8, 8, 12, 0.55);
  backdrop-filter: blur(3px);
  display: grid;
  place-items: center;
  padding: 20px;
}
.dialog {
  width: min(560px, 100%);
  max-height: 90vh;
  overflow: auto;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: var(--r-lg);
  padding: 18px 20px 16px;
  box-shadow: 0 24px 60px -24px rgba(0, 0, 0, 0.5);
}
.d-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.who {
  display: flex;
  gap: 10px;
  align-items: flex-start;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-top: 7px;
}
.d-head h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.d-head p {
  margin: 4px 0 0;
  font-size: 11.5px;
}
.tabs {
  display: flex;
  gap: 6px;
  margin: 14px 0 12px;
}
.tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 12px;
  font-size: 12.5px;
  color: var(--text-2);
  background: transparent;
  border: 1px solid var(--line);
  border-radius: var(--r-full);
  cursor: pointer;
}
.tab.on {
  color: var(--on-accent, #fff);
  background: var(--accent);
  border-color: var(--accent);
}
.qr-wrap {
  display: grid;
  grid-template-columns: 232px minmax(0, 1fr);
  gap: 18px;
  align-items: start;
  margin: 6px 0 4px;
}
.web-wrap {
  margin: 6px 0 4px;
}
.preparing {
  margin: 18px 0;
  font-size: 12px;
  color: var(--text-3);
}
.web-side {
  display: grid;
  gap: 8px;
  justify-items: start;
}
.web-btn {
  margin-top: 4px;
}
.qr-state.idle .bulb {
  background: var(--text-3);
}
.qr-box {
  position: relative;
  width: 232px;
  height: 232px;
  border-radius: var(--r-md);
  background: #fff;
  padding: 10px;
  box-shadow: inset 0 0 0 1px var(--line);
}
.qr-svg :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}
.qr-img {
  width: 100%;
  height: 100%;
  display: block;
  object-fit: contain;
  image-rendering: pixelated;
}
.qr-box.expired .qr-svg,
.qr-box.error .qr-svg,
.qr-box.expired .qr-img,
.qr-box.error .qr-img {
  opacity: 0.15;
  filter: grayscale(1);
}
.qr-mask {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
}
.qr-side {
  min-width: 0;
}
.qr-state {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 6px;
  font-size: 13px;
  font-weight: 500;
}
.bulb {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-3);
}
.qr-state.waiting .bulb {
  background: var(--accent);
  animation: pulse 1.6s infinite;
}
.qr-state.scanned .bulb {
  background: var(--qq, #23a166);
}
.qr-state.confirmed .bulb {
  background: var(--qq, #23a166);
}
.qr-state.expired .bulb,
.qr-state.error .bulb {
  background: var(--netease, #e23b3b);
}
@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.25;
  }
}
.countdown {
  font-size: 11.5px;
  color: var(--text-3);
  margin: 6px 0 0;
}
.steps {
  margin: 12px 0 0;
  padding-left: 16px;
  font-size: 11.5px;
  color: var(--text-3);
  line-height: 1.9;
}
.cookie-wrap {
  display: grid;
  gap: 10px;
}
.hint {
  margin: 0;
  font-size: 12px;
  color: var(--text-2);
  line-height: 1.7;
}
.hint code {
  font-size: 11.5px;
  padding: 1px 4px;
  border-radius: 4px;
  background: var(--surface-2);
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  font-size: inherit;
  cursor: pointer;
  text-decoration: underline;
}
.cookie-input {
  width: 100%;
  resize: vertical;
  font-family: var(--mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.6;
}
.d-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--line);
}
.acts {
  display: flex;
  gap: 8px;
}
.tiny {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
}
.err {
  margin: 10px 0 0;
  font-size: 12px;
  color: var(--netease, #e23b3b);
}
.small {
  font-size: 11.5px;
  margin: 0;
}
@media (max-width: 620px) {
  .qr-wrap {
    grid-template-columns: minmax(0, 1fr);
    justify-items: center;
  }
  .qr-side {
    width: 100%;
  }
  .d-foot {
    flex-direction: column;
    align-items: stretch;
  }
  .acts {
    justify-content: flex-end;
  }
}
</style>
