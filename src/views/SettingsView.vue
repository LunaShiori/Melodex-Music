<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import AccountPanel from '@/components/AccountPanel.vue'
import Icon from '@/components/Icon.vue'
import { renderNaming } from '@/lib/naming'
import { transport } from '@/lib/transport'
import { ALL_SOURCES, SOURCE_META } from '@/lib/types'
import type { Settings, SourceId } from '@/lib/types'
import { useSettingsStore } from '@/stores/settings'
import { useUpdateStore } from '@/stores/update'

const store = useSettingsStore()
const updateStore = useUpdateStore()
const defaultDir = ref('')
const saving = ref(false)
const version = __APP_VERSION__

async function checkUpdate() {
  await updateStore.check().catch(() => {
    /* 错误已经写进 updateStore.error，界面就地展示 */
  })
}

const settings = computed(() => store.settings)

const preview = computed(() =>
  renderNaming(settings.value?.namingTemplate ?? '', {
    title: '晴天',
    artists: ['周杰伦'],
    album: '叶惠美',
    source: 'netease',
    bitrate: settings.value?.bitrate ?? 320,
    index: 1,
    artistSeparator: settings.value?.artistSeparator,
  }),
)

const BITRATES = [
  { value: 999, label: '无损' },
  { value: 320, label: '320k' },
  { value: 192, label: '192k' },
  { value: 128, label: '128k' },
]

async function update(patch: Partial<Settings>) {
  if (!settings.value) return
  saving.value = true
  await store.save({ ...settings.value, ...patch })
  window.setTimeout(() => (saving.value = false), 400)
}

function toggleSource(source: SourceId) {
  if (!settings.value) return
  const has = settings.value.sources.includes(source)
  const next = has
    ? settings.value.sources.filter((item) => item !== source)
    : [...settings.value.sources, source]
  if (!next.length) return
  void update({ sources: next })
}

function toggleFallback(value: number) {
  if (!settings.value) return
  const has = settings.value.fallbackBitrates.includes(value)
  const next = has
    ? settings.value.fallbackBitrates.filter((item) => item !== value)
    : [...settings.value.fallbackBitrates, value]
  if (!next.length) return
  void update({ fallbackBitrates: next })
}

onMounted(async () => {
  defaultDir.value = await transport.defaultDownloadDir()
})
</script>

<template>
  <div class="view">
    <header class="head">
      <div>
        <h1>设置</h1>
        <p class="muted">改动即时生效并持久化，无需手动保存。</p>
      </div>
      <span class="saved" :class="{ show: saving }">
        <Icon name="check" :size="13" />
        已保存
      </span>
    </header>

    <template v-if="settings">
      <AccountPanel />

      <section class="card">
        <h2>下载</h2>

        <div class="row">
          <div class="label">
            <p>下载目录</p>
            <small>留空则使用系统默认目录</small>
          </div>
          <div class="control wide">
            <input
              class="input"
              :value="settings.downloadDir"
              :placeholder="defaultDir"
              @change="update({ downloadDir: ($event.target as HTMLInputElement).value })"
            />
            <button class="btn btn-sm" type="button" @click="update({ downloadDir: '' })">
              用默认
            </button>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>同时下载数</p>
            <small>建议 2–4，过高容易被音源限速</small>
          </div>
          <div class="control">
            <input
              class="range"
              type="range"
              min="1"
              max="8"
              :value="settings.concurrency"
              @input="update({ concurrency: Number(($event.target as HTMLInputElement).value) })"
            />
            <span class="value mono">{{ settings.concurrency }}</span>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>重复文件</p>
            <small>目标文件已存在时的处理方式</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="option in [
                  { value: 'keepBoth', label: '保留两份' },
                  { value: 'overwrite', label: '覆盖' },
                  { value: 'skip', label: '跳过' },
                ]"
                :key="option.value"
                type="button"
                :class="{ on: settings.duplicatePolicy === option.value }"
                @click="update({ duplicatePolicy: option.value as Settings['duplicatePolicy'] })"
              >
                {{ option.label }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>音质</h2>

        <div class="row">
          <div class="label">
            <p>目标音质</p>
            <small>音源不提供时会按下方降级链尝试</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="option in BITRATES"
                :key="option.value"
                type="button"
                :class="{ on: settings.bitrate === option.value }"
                @click="update({ bitrate: option.value })"
              >
                {{ option.label }}
              </button>
            </div>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>降级链</p>
            <small>目标音质取不到链接时，依次尝试</small>
          </div>
          <div class="control">
            <button
              v-for="value in [320, 192, 128]"
              :key="value"
              class="chip chip-btn"
              :class="{ 'is-on': settings.fallbackBitrates.includes(value) }"
              type="button"
              @click="toggleFallback(value)"
            >
              {{ value }}k
            </button>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>优先音源</p>
            <small>同一首歌在多个平台都能取到时按此排序</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button type="button" :class="{ on: !settings.preferSource }" @click="update({ preferSource: null })">
                自动
              </button>
              <button
                v-for="source in ALL_SOURCES"
                :key="source"
                type="button"
                :class="{ on: settings.preferSource === source }"
                @click="update({ preferSource: source })"
              >
                {{ SOURCE_META[source].short }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>文件与标签</h2>

        <div class="row">
          <div class="label">
            <p>命名模板</p>
            <small>可用占位符：{artist} {title} {album} {source} {bitrate} {index:02}</small>
          </div>
          <div class="control wide">
            <input
              class="input"
              :value="settings.namingTemplate"
              @change="update({ namingTemplate: ($event.target as HTMLInputElement).value })"
            />
          </div>
        </div>

        <div class="row preview-row">
          <div class="label"><p class="muted">预览</p></div>
          <div class="control wide">
            <code class="preview mono">{{ preview }}.mp3</code>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>多歌手分隔符</p>
            <small>写入标签与文件名时使用</small>
          </div>
          <div class="control">
            <input
              class="input narrow"
              :value="settings.artistSeparator"
              @change="update({ artistSeparator: ($event.target as HTMLInputElement).value })"
            />
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>写入 ID3 标签</p>
            <small>标题 / 歌手 / 专辑 / 音源来源</small>
          </div>
          <div class="control">
            <button
              class="switch"
              type="button"
              :class="{ on: settings.writeTags }"
              @click="update({ writeTags: !settings.writeTags })"
            />
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>内嵌封面</p>
            <small>从音源抓取专辑封面写入音频</small>
          </div>
          <div class="control">
            <button
              class="switch"
              type="button"
              :class="{ on: settings.embedCover }"
              :disabled="!settings.writeTags"
              @click="update({ embedCover: !settings.embedCover })"
            />
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>另存 .lrc 歌词</p>
            <small>与音频同目录，文件名相同</small>
          </div>
          <div class="control">
            <button
              class="switch"
              type="button"
              :class="{ on: settings.saveLyric }"
              @click="update({ saveLyric: !settings.saveLyric })"
            />
          </div>
        </div>
      </section>

      <section class="card">
        <h2>外观</h2>
        <div class="row">
          <div class="label">
            <p>主题</p>
            <small>默认跟随系统</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="option in [
                  { value: 'light', label: '浅色' },
                  { value: 'dark', label: '深色' },
                  { value: 'system', label: '跟随系统' },
                ]"
                :key="option.value"
                type="button"
                :class="{ on: settings.theme === option.value }"
                @click="update({ theme: option.value as Settings['theme'] })"
              >
                {{ option.label }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>音源</h2>
        <div v-for="source in ALL_SOURCES" :key="source" class="row">
          <div class="label">
            <p>{{ SOURCE_META[source].label }}</p>
            <small>{{ SOURCE_META[source].home }}</small>
          </div>
          <div class="control">
            <button
              class="switch"
              type="button"
              :class="{ on: settings.sources.includes(source) }"
              @click="toggleSource(source)"
            />
          </div>
        </div>
      </section>

      <section class="card about">
        <h2>关于</h2>
        <div class="about-grid">
          <div>
            <p class="k">Melodex</p>
            <p class="v muted">v{{ version }} · Apache-2.0</p>
          </div>
          <div>
            <p class="k">运行模式</p>
            <p class="v muted">{{ transport.mode === 'tauri' ? '本地 Rust 引擎' : '浏览器演示数据' }}</p>
          </div>
          <div>
            <p class="k">开源协议</p>
            <p class="v muted">仅用于学习与研究，请遵守各音源版权规定</p>
          </div>
        </div>

        <div class="update-row">
          <div>
            <p class="k">在线更新</p>
            <p class="v muted">
              <template v-if="updateStore.checking">正在检查 GitHub Releases…</template>
              <template v-else-if="updateStore.error">检查失败：{{ updateStore.error }}</template>
              <template v-else-if="updateStore.info?.available">
                发现新版本 <span class="mono">{{ updateStore.info.latest }}</span>，可在弹窗中下载安装包
              </template>
              <template v-else-if="updateStore.info">已是最新版本</template>
              <template v-else>安装包由 GitHub Releases 分发，启动时会静默检查一次</template>
            </p>
          </div>
          <button class="btn btn-sm" type="button" :disabled="updateStore.checking" @click="checkUpdate">
            <Icon name="refresh" :size="14" />
            检查更新
          </button>
        </div>

        <p class="disclaimer">
          Melodex 不提供任何音乐内容，所有音频均来自第三方平台公开接口；请确保你对下载的内容拥有合法使用权。
        </p>
      </section>
    </template>
  </div>
</template>

<style scoped>
.view {
  padding: 26px 26px 48px;
  max-width: 860px;
  margin: 0 auto;
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  margin-bottom: 18px;
}
.head h1 {
  margin: 0;
  font-size: 22px;
  font-weight: 600;
}
.head p {
  margin: 5px 0 0;
  font-size: 12.5px;
}

.saved {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--accent-2);
  opacity: 0;
  transition: opacity 200ms var(--ease);
}
.saved.show {
  opacity: 1;
}

.card {
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  background: var(--surface);
  padding: 4px 16px;
  margin-bottom: 14px;
}
.card h2 {
  margin: 14px 0 4px;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--text-2);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
}
.card h2 + .row {
  border-top: none;
}
.label p {
  margin: 0;
  font-size: 13px;
}
.label small {
  display: block;
  margin-top: 2px;
  font-size: 11px;
  color: var(--text-3);
}
.control {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
}
.control.wide {
  flex: 1;
  max-width: 420px;
  justify-content: flex-end;
}

.input {
  height: 32px;
  min-width: 0;
  flex: 1;
  padding: 0 10px;
  border-radius: var(--r-sm);
  border: 1px solid var(--line);
  background: var(--surface-2);
  font-size: 12.5px;
  outline: none;
}
.input:focus {
  border-color: color-mix(in srgb, var(--accent) 50%, transparent);
}
.input.narrow {
  width: 70px;
  flex: 0 0 auto;
}

.range {
  width: 150px;
  accent-color: var(--accent);
}
.value {
  width: 18px;
  text-align: center;
  font-size: 12px;
  color: var(--text-2);
}

.segmented {
  display: flex;
  gap: 2px;
  padding: 3px;
  border-radius: var(--r-sm);
  background: var(--surface-2);
}
.segmented button {
  height: 26px;
  padding: 0 11px;
  border-radius: 7px;
  font-size: 12px;
  color: var(--text-2);
  transition: all var(--dur) var(--ease);
  white-space: nowrap;
}
.segmented button.on {
  background: var(--surface);
  color: var(--text);
  font-weight: 500;
  box-shadow: var(--shadow-sm);
}

.chip {
  cursor: pointer;
}

.switch {
  position: relative;
  width: 40px;
  height: 23px;
  border-radius: var(--r-full);
  background: var(--surface-3);
  transition: background var(--dur) var(--ease);
}
.switch::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 19px;
  height: 19px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  transition: transform var(--dur) var(--ease);
}
.switch.on {
  background: var(--accent);
}
.switch.on::after {
  transform: translateX(17px);
}
.switch:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.preview-row .preview {
  font-size: 12px;
  color: var(--text);
  background: var(--surface-2);
  padding: 6px 10px;
  border-radius: var(--r-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

.about-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 14px;
  padding: 10px 0 4px;
  border-top: 1px solid var(--line);
}
.k {
  margin: 0;
  font-size: 12px;
  font-weight: 500;
}
.v {
  margin: 2px 0 0;
  font-size: 11.5px;
}
.update-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-top: 14px;
  padding: 12px 14px;
  border-radius: var(--r-sm);
  background: var(--surface-2);
}
.update-row .v {
  max-width: 520px;
  line-height: 1.5;
}
.disclaimer {
  margin: 8px 0 14px;
  font-size: 11px;
  line-height: 1.6;
  color: var(--text-3);
}

@media (max-width: 860px) {
  .view {
    padding: 18px 14px 30px;
  }
  .row {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .control.wide {
    width: 100%;
    max-width: none;
  }
}
</style>
