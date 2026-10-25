<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import Icon from './Icon.vue'
import { formatBytes } from '@/lib/format'
import { transport } from '@/lib/transport'
import { useUpdateStore } from '@/stores/update'

const update = useUpdateStore()
const open = ref(false)
/** 手动挑选安装包 / 打开发布页，默认收起 */
const more = ref(false)
const failed = ref('')

const version = __APP_VERSION__
const info = computed(() => update.info)

const KIND_LABEL: Record<string, string> = {
  nsis: 'Windows 安装包 · exe',
  msi: 'Windows 安装包 · msi',
  apk: 'Android 安装包',
  aab: 'Android 应用包',
  dmg: 'macOS 安装包',
  appimage: 'Linux 安装包',
  zip: '压缩包',
  other: '附件',
}

/** 按当前平台把最合适的安装包排在最前 */
const assets = computed(() => {
  const list = info.value?.assets ?? []
  const prefer = navigator.userAgent.includes('Android') ? 'apk' : 'nsis'
  return [...list].sort((a, b) => Number(b.kind === prefer) - Number(a.kind === prefer))
})

/** 主按钮文案与可用性 */
const primaryLabel = computed(() => {
  switch (update.stage) {
    case 'downloading':
      return update.totalBytes
        ? `下载中 ${update.percent}%`
        : `下载中 ${formatBytes(update.downloaded)}`
    case 'ready':
      return '立即安装并重启'
    case 'installing':
      return '正在退出…'
    default:
      return '立即更新'
  }
})

const progressHint = computed(() => {
  if (update.stage !== 'downloading') return ''
  if (!update.totalBytes) return `已下载 ${formatBytes(update.downloaded)}`
  return `${formatBytes(update.downloaded)} / ${formatBytes(update.totalBytes)}`
})

watch(open, (value) => {
  if (!value) {
    failed.value = ''
    more.value = false
  }
})

async function primary() {
  failed.value = ''
  if (update.stage === 'ready') {
    await update.install()
    return
  }
  if (update.stage === 'idle') {
    await update.download()
  }
}

async function downloadSpecific(name: string) {
  failed.value = ''
  await update.download(name)
}

async function openUrl(url: string) {
  if (!url) return
  failed.value = ''
  try {
    await transport.openUrl(url)
  } catch (err) {
    failed.value = err instanceof Error ? err.message : String(err)
  }
}
</script>

<template>
  <!-- 发现新版本时的悬浮提示条 -->
  <Transition name="fade">
    <div v-if="update.showBanner && !open" class="notice">
      <span class="ico"><Icon name="cloud" :size="15" /></span>
      <div class="txt">
        <p class="title">发现新版本 <span class="mono">{{ info?.latest }}</span></p>
        <p class="sub muted">当前 v{{ version }}，可直接在应用内更新</p>
      </div>
      <button class="btn btn-primary btn-sm" type="button" @click="open = true">更新</button>
      <button class="icon-btn" type="button" title="稍后提醒" @click="update.dismiss()">
        <Icon name="close" :size="14" />
      </button>
    </div>
  </Transition>

  <Teleport to="body">
    <Transition name="fade">
      <div v-if="open && info" class="mask" @click.self="!update.busy && (open = false)">
        <div class="dialog" role="dialog" aria-modal="true">
          <header class="d-head">
            <div>
              <h3>Melodex {{ info.latest }}</h3>
              <p class="muted">
                当前版本 v{{ info.current }}
                <span v-if="info.publishedAt"> · 发布于 {{ info.publishedAt.slice(0, 10) }}</span>
              </p>
            </div>
            <button
              class="icon-btn"
              type="button"
              :disabled="update.busy"
              @click="open = false"
            >
              <Icon name="close" :size="16" />
            </button>
          </header>

          <pre class="notes">{{ info.notes || '本次更新没有附带说明。' }}</pre>

          <!-- 下载进度 -->
          <div v-if="update.stage === 'downloading'" class="progress">
            <div class="bar">
              <span class="fill" :style="{ width: `${update.percent}%` }" />
            </div>
            <p class="hint muted small">{{ progressHint }}</p>
          </div>

          <!-- 下载完成 -->
          <p v-else-if="update.stage === 'ready'" class="ready small">
            安装包已下载完成，点「立即安装并重启」即可完成升级。
          </p>
          <p v-else-if="update.stage === 'installing'" class="ready small">
            正在启动安装程序，Melodex 即将退出，安装完成后会自动重新打开。
          </p>

          <p v-if="update.installError || failed" class="err">
            {{ update.installError || failed }}
          </p>

          <!-- 备用入口：手动挑安装包 / 去发布页 -->
          <button class="more-toggle" type="button" @click="more = !more">
            <Icon name="chevron" :size="13" :class="more ? 'flip' : ''" />
            手动下载与其他格式
          </button>

          <div v-if="more" class="more">
            <div v-if="assets.length" class="assets">
              <button
                v-for="asset in assets"
                :key="asset.name"
                class="asset"
                type="button"
                :disabled="update.busy"
                @click="downloadSpecific(asset.name)"
              >
                <span class="kind">{{ KIND_LABEL[asset.kind] ?? KIND_LABEL.other }}</span>
                <span class="name mono">{{ asset.name }}</span>
                <span class="size mono">{{ formatBytes(asset.size) }}</span>
                <Icon name="download" :size="15" />
              </button>
            </div>
            <p v-else class="muted small">这次发布没有附带安装包。</p>
            <button class="link-btn" type="button" @click="openUrl(info.pageUrl)">
              在浏览器中打开发布页
            </button>
          </div>

          <footer class="d-foot">
            <button class="btn btn-ghost" type="button" :disabled="update.busy" @click="open = false">
              稍后提醒
            </button>
            <button class="btn btn-primary" type="button" :disabled="update.busy" @click="primary">
              {{ primaryLabel }}
            </button>
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.notice {
  position: fixed;
  left: calc(var(--sidebar) + 22px);
  bottom: calc(var(--player) + 16px);
  z-index: 60;
  display: flex;
  align-items: center;
  gap: 10px;
  max-width: 420px;
  padding: 10px 12px;
  border-radius: var(--r-md);
  background: var(--surface);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-md);
}
.ico {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  flex: 0 0 auto;
  border-radius: 9px;
  color: var(--accent);
  background: var(--active);
}
.txt {
  min-width: 0;
  margin-right: 4px;
}
.title {
  margin: 0;
  font-size: 12.5px;
  font-weight: 500;
}
.sub {
  margin: 2px 0 0;
  font-size: 11px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mask {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgba(8, 8, 14, 0.44);
  backdrop-filter: blur(2px);
}
.dialog {
  width: min(520px, 100%);
  max-height: min(80vh, 660px);
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px 20px 16px;
  border-radius: var(--r-lg);
  background: var(--surface);
  border: 1px solid var(--line);
  box-shadow: 0 24px 60px -24px rgba(0, 0, 0, 0.5);
}
.d-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.d-head h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.d-head p {
  margin: 3px 0 0;
  font-size: 11.5px;
}
.notes {
  margin: 0;
  padding: 12px;
  max-height: 200px;
  overflow: auto;
  border-radius: var(--r-sm);
  background: var(--surface-2);
  font-size: 12px;
  line-height: 1.7;
  color: var(--text-2);
  white-space: pre-wrap;
  font-family: inherit;
}

.progress {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.bar {
  height: 6px;
  border-radius: 999px;
  background: var(--surface-2);
  overflow: hidden;
}
.fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 160ms linear;
}
.hint {
  margin: 0;
  font-variant-numeric: tabular-nums;
}
.ready {
  margin: 0;
  color: var(--text-2);
}

.more-toggle {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  align-self: flex-start;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-3);
  font-size: 11.5px;
  cursor: pointer;
}
.more-toggle:hover {
  color: var(--text);
}
/* 展开时箭头朝上：同一个图标旋转 180°，省掉一个图标定义 */
.flip {
  transform: rotate(180deg);
}
.more {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.assets {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.asset {
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr) auto 18px;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
  border-radius: var(--r-sm);
  border: 1px solid var(--line);
  background: var(--surface);
  text-align: left;
  font-size: 11.5px;
  transition: all var(--dur) var(--ease);
}
.asset:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  background: var(--hover);
}
.asset:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.kind {
  color: var(--text-2);
}
.name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text);
}
.size {
  color: var(--text-3);
  font-size: 10.5px;
}
.link-btn {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  font-size: 11.5px;
  text-align: left;
  cursor: pointer;
}
.link-btn:hover {
  text-decoration: underline;
}
.err {
  margin: 0;
  font-size: 11.5px;
  color: var(--danger);
}
.small {
  font-size: 11.5px;
}
.d-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

@media (max-width: 860px) {
  .notice {
    left: 12px;
    right: 12px;
    bottom: calc(var(--player) + 64px);
    max-width: none;
  }
  .asset {
    grid-template-columns: minmax(0, 1fr) auto 16px;
  }
  .kind {
    display: none;
  }
  .dialog {
    padding: 16px 14px 14px;
  }
  .d-foot {
    flex-direction: column-reverse;
  }
  .d-foot .btn {
    width: 100%;
  }
}
</style>
