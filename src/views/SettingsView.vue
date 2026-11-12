<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import AccountPanel from '@/components/AccountPanel.vue'
import Icon from '@/components/Icon.vue'
import {
  formatOffset,
  LYRIC_FONT_MAX,
  LYRIC_FONT_MIN,
  LYRIC_MODES,
  LYRIC_THEMES,
} from '@/lib/lyric-style'
import { sourceName, sourceShort, useI18n } from '@/lib/i18n'
import type { LocaleChoice } from '@/lib/i18n'
import { renderNaming } from '@/lib/naming'
import { nominalBitrate, QUALITY_LEVELS, qualityRank } from '@/lib/quality'
import { ACCENTS } from '@/lib/theme'
import { transport } from '@/lib/transport'
import { ALL_SOURCES, SOURCE_META } from '@/lib/types'
import type {
  PlatformQuality,
  ProxyScopes,
  QualityLevel,
  Settings,
  SourceId,
} from '@/lib/types'
import { UI_STYLES } from '@/lib/ui-style'
import { useLyricStore } from '@/stores/lyric'
import { useOnboardingStore } from '@/stores/onboarding'
import { useSettingsStore } from '@/stores/settings'
import { useUpdateStore } from '@/stores/update'

const store = useSettingsStore()
const { t, choice: localeChoice, setLocale } = useI18n()
const lyricStore = useLyricStore()
const updateStore = useUpdateStore()
const onboarding = useOnboardingStore()
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
    bitrate: nominalBitrate(settings.value?.quality ?? 'lossless'),
    index: 1,
    artistSeparator: settings.value?.artistSeparator,
  }),
)

/* ---------- 音质：全局默认 + 分平台覆盖 ---------- */

/** 各平台的档位表，由后端唯一维护，界面据此渲染平台自己的叫法 */
const qualities = ref<PlatformQuality[]>([])

/** 该平台的目标等级：分平台设置优先于全局默认 */
function targetLevel(source: SourceId): QualityLevel {
  return settings.value?.qualityOverrides[source] ?? settings.value?.quality ?? 'lossless'
}

/**
 * 该平台实际会用的档位。目标超出平台上限时落到它的最高档——
 * 界面上必须高亮这一档，否则用户会以为酷我也能下无损。
 */
function shownTier(source: SourceId) {
  const platform = qualities.value.find((item) => item.source === source)
  if (!platform?.items.length) return null
  const target = qualityRank(targetLevel(source))
  return (
    platform.items.find((tier) => qualityRank(tier.level) <= target) ??
    platform.items[platform.items.length - 1]
  )
}

function setQuality(source: SourceId, level: QualityLevel) {
  if (!settings.value) return
  const overrides = { ...settings.value.qualityOverrides }
  // 与全局默认一致时不必留覆盖项，配置表越短越好
  if (level === settings.value.quality) delete overrides[source]
  else overrides[source] = level
  void update({ qualityOverrides: overrides })
}

function resetQuality(source: SourceId) {
  if (!settings.value) return
  const overrides = { ...settings.value.qualityOverrides }
  delete overrides[source]
  void update({ qualityOverrides: overrides })
}

function tierClass(source: SourceId, level: QualityLevel) {
  return { on: shownTier(source)?.level === level }
}

async function update(patch: Partial<Settings>) {
  if (!settings.value) return
  saving.value = true
  await store.save({ ...settings.value, ...patch })
  window.setTimeout(() => (saving.value = false), 400)
}

async function browseDir() {
  const dir = await transport.pickDownloadDir()
  if (dir) void update({ downloadDir: dir })
}

type ProxyScheme = 'direct' | 'http' | 'https' | 'socks5' | 'socks5h' | 'socks4'

const PROXY_SCHEMES: { value: ProxyScheme; label: string }[] = [
  { value: 'direct', label: 'settings.proxy.direct' },
  { value: 'http', label: 'HTTP' },
  { value: 'https', label: 'HTTPS' },
  { value: 'socks5', label: 'SOCKS5' },
  { value: 'socks5h', label: 'SOCKS5H' },
  { value: 'socks4', label: 'SOCKS4' },
]

/** 核心只认这几种协议头，其余一律当直连，所以下拉里不给别的选项。 */
const PROXY_SCOPE_ITEMS: { key: keyof ProxyScopes; label: string; hint: string }[] = [
  { key: 'search', label: 'settings.scope.search', hint: 'settings.scope.searchHint' },
  { key: 'link', label: 'settings.scope.link', hint: 'settings.scope.linkHint' },
  { key: 'download', label: 'settings.scope.download', hint: 'settings.scope.downloadHint' },
  { key: 'update', label: 'settings.scope.update', hint: 'settings.scope.updateHint' },
]

const proxyScheme = ref<ProxyScheme>('direct')
const proxyAddr = ref('')

/** 把存下来的整串拆成「协议 + 地址」两个控件：http://127.0.0.1:7890 → http / 127.0.0.1:7890 */
function splitProxy(raw: string): { scheme: ProxyScheme; addr: string } {
  const text = raw.trim()
  if (!text) return { scheme: 'direct', addr: '' }
  const matched = /^([a-z0-9]+):\/\/(.*)$/i.exec(text)
  // 少了协议头：地址照样回填，选个协议就能修好
  if (!matched) return { scheme: 'direct', addr: text }
  const scheme = matched[1].toLowerCase()
  const known = PROXY_SCHEMES.find((item) => item.value === scheme)
  return { scheme: known ? known.value : 'http', addr: matched[2] }
}

watch(
  () => settings.value?.proxy ?? '',
  (raw) => {
    if (!raw.trim()) {
      proxyScheme.value = 'direct'
      return
    }
    const next = splitProxy(raw)
    proxyScheme.value = next.scheme
    proxyAddr.value = next.addr
  },
  { immediate: true },
)

/** 协议 + 地址一起决定存什么；直连或地址为空都存空字符串。 */
function commitProxy() {
  const addr = proxyAddr.value.trim()
  const next = proxyScheme.value === 'direct' || !addr ? '' : `${proxyScheme.value}://${addr}`
  if (next !== (settings.value?.proxy ?? '')) void update({ proxy: next })
}

function toggleScope(key: keyof ProxyScopes, on: boolean) {
  const current = settings.value?.proxyScopes
  if (!current) return
  void update({ proxyScopes: { ...current, [key]: on } })
}

/** 限速上限与核心 settings.rs 的 sanitize 对齐：1 GB/s。 */
const SPEED_LIMIT_MAX = 1048576

/** 代理地址认这几个前缀；核心那边用 reqwest 解析，对不上就退回直连。 */
/** 协议已经选过了，这里只挑「地址本身不对劲」的情况提示，别在用户打字途中烦人。 */
const proxyHint = computed(() => {
  if (proxyScheme.value === 'direct') return ''
  const addr = proxyAddr.value.trim()
  if (!addr) return ''
  if (/\s/.test(addr)) return t('settings.proxy.space')
  if (!/:[0-9]{1,5}$/.test(addr)) return t('settings.proxy.port')
  return ''
})

/**
 * 限速输入框：允许直接敲数字，也允许清空（清空 = 不限）。
 * 超过上限时按上限存，并把输入框回写成收敛后的值，避免屏幕上显示一个存不下的数。
 */
function onSpeedLimitInput(event: Event) {
  const input = event.target as HTMLInputElement
  const raw = input.value.trim()
  if (raw === '') {
    void update({ speedLimitKbps: 0 })
    return
  }
  const parsed = Number(raw)
  if (!Number.isFinite(parsed)) return
  const clamped = Math.max(0, Math.min(SPEED_LIMIT_MAX, Math.round(parsed)))
  void update({ speedLimitKbps: clamped })
  if (input.value !== String(clamped)) input.value = String(clamped)
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

onMounted(async () => {
  defaultDir.value = await transport.defaultDownloadDir()
  qualities.value = await transport.platformQualities().catch(() => [])
})
</script>

<template>
  <div class="view">
    <header class="head">
      <div>
        <h1>{{ t('settings.a1') }}</h1>
        <p class="muted">{{ t('settings.a2') }}</p>
      </div>
      <span class="saved" :class="{ show: saving }">
        <Icon name="check" :size="13" />{{ t('settings.a3') }}</span>
    </header>

    <template v-if="settings">
      <AccountPanel />

      <!-- 手机上没有侧边栏，这里给个入口进「更多平台」 -->
      <RouterLink class="card link-card" to="/platforms">
        <div>
          <h2>{{ t('nav.platforms') }}</h2>
          <p class="muted">{{ t('settings.platformsHint') }}</p>
        </div>
        <Icon name="external" :size="16" />
      </RouterLink>

      <section class="card">
        <h2>{{ t('settings.language.title') }}</h2>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.language.title') }}</p>
            <small>{{ t('settings.language.hint') }}</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="option in [
                  { value: 'auto', label: t('settings.language.auto') },
                  { value: 'zh', label: t('settings.language.zh') },
                  { value: 'en', label: t('settings.language.en') },
                ]"
                :key="option.value"
                type="button"
                :class="{ on: localeChoice === option.value }"
                @click="setLocale(option.value as LocaleChoice)"
              >
                {{ t(option.label) }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>{{ t('settings.a4') }}</h2>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a5') }}</p>
            <small>{{ t('settings.a6') }}</small>
          </div>
          <div class="control wide">
            <div class="proxy-line">
              <select
                class="select"
                :value="proxyScheme"
                @change="proxyScheme = ($event.target as HTMLSelectElement).value as ProxyScheme; commitProxy()"
              >
                <option v-for="item in PROXY_SCHEMES" :key="item.value" :value="item.value">
                  {{ t(item.label) }}
                </option>
              </select>
              <input
                class="input"
                spellcheck="false"
                autocomplete="off"
                placeholder="127.0.0.1:7890"
                :disabled="proxyScheme === 'direct'"
                :value="proxyAddr"
                @input="proxyAddr = ($event.target as HTMLInputElement).value"
                @change="commitProxy()"
              />
            </div>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a7') }}</p>
            <small>{{ t('settings.a8') }}</small>
          </div>
          <div class="control wide">
            <div class="scope-list">
              <label
                v-for="item in PROXY_SCOPE_ITEMS"
                :key="item.key"
                class="scope"
                :title="item.hint"
              >
                <input
                  type="checkbox"
                  :checked="settings.proxyScopes?.[item.key] ?? true"
                  @change="toggleScope(item.key, ($event.target as HTMLInputElement).checked)"
                />
                <span>{{ t(item.label) }}</span>
              </label>
            </div>
          </div>
        </div>
        <p v-if="proxyHint" class="proxy-hint">{{ proxyHint }}</p>
      </section>

      <section class="card">
        <h2>{{ t('settings.a9') }}</h2>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a10') }}</p>
            <small>{{ t('settings.a11') }}</small>
          </div>
          <div class="control wide">
            <input
              class="input"
              :value="settings.downloadDir"
              :placeholder="defaultDir"
              @change="update({ downloadDir: ($event.target as HTMLInputElement).value })"
            />
            <button class="btn btn-sm" type="button" @click="browseDir">{{ t('settings.a12') }}</button>
            <button class="btn btn-sm" type="button" @click="update({ downloadDir: '' })">{{ t('settings.a13') }}</button>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a14') }}</p>
            <small>{{ t('settings.a15') }}</small>
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
            <p>{{ t('settings.a16') }}</p>
            <small>{{ t('settings.a17') }}</small>
          </div>
          <div class="control speed-control">
            <input
              class="range"
              type="range"
              min="0"
              max="10240"
              step="128"
              :value="Math.min(settings.speedLimitKbps, 10240)"
              @input="update({ speedLimitKbps: Number(($event.target as HTMLInputElement).value) })"
            />
            <span class="speed-field">
              <input
                class="input speed-input"
                type="number"
                inputmode="numeric"
                min="0"
                :max="SPEED_LIMIT_MAX"
                step="1"
                :placeholder="t('settings.b70')"
                :value="settings.speedLimitKbps || ''"
                @input="onSpeedLimitInput"
                @change="onSpeedLimitInput"
              />
              <span class="speed-unit">KB/s</span>
            </span>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a18') }}</p>
            <small>{{ t('settings.a19') }}</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="option in [
                  { value: 'keepBoth', label: 'settings.dupe.keepBoth' },
                  { value: 'overwrite', label: 'settings.dupe.overwrite' },
                  { value: 'skip', label: 'settings.dupe.skip' },
                ]"
                :key="option.value"
                type="button"
                :class="{ on: settings.duplicatePolicy === option.value }"
                @click="update({ duplicatePolicy: option.value as Settings['duplicatePolicy'] })"
              >
                {{ t(option.label) }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>{{ t('settings.a20') }}</h2>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a21') }}</p>
            <small>{{ t('settings.a22') }}</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="level in QUALITY_LEVELS"
                :key="level.id"
                type="button"
                :class="{ on: settings.quality === level.id }"
                @click="update({ quality: level.id })"
              >
                {{ t(level.label) }}
              </button>
            </div>
          </div>
        </div>

        <div class="quality-grid">
          <article v-for="platform in qualities" :key="platform.source" class="platform">
            <header class="platform-head">
              <span class="dot" :style="{ background: SOURCE_META[platform.source].color }" />
              <strong>{{ platform.label }}</strong>
              <button
                v-if="settings.qualityOverrides[platform.source]"
                class="link"
                type="button"
                @click="resetQuality(platform.source)"
              >{{ t('settings.a23') }}</button>
              <span v-else class="muted tiny">{{ t('settings.a23') }}</span>
            </header>
            <p class="muted tiny note">{{ platform.note }}</p>
            <div class="tiers">
              <button
                v-for="tier in platform.items"
                :key="tier.code"
                class="tier"
                :class="tierClass(platform.source, tier.level)"
                type="button"
                @click="setQuality(platform.source, tier.level)"
              >
                <span class="tier-name">
                  {{ tier.label }}
                  <span v-if="tier.vip" class="vip">{{ t('settings.a24') }}</span>
                </span>
                <span class="tier-detail mono">{{ tier.detail }}</span>
              </button>
            </div>
          </article>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a25') }}</p>
            <small>{{ t('settings.a26') }}</small>
          </div>
          <div class="control">
            <button
              class="switch"
              type="button"
              :class="{ on: settings.autoFallback }"
              :aria-pressed="settings.autoFallback"
              @click="update({ autoFallback: !settings.autoFallback })"
            />
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a27') }}</p>
            <small>{{ t('settings.a28') }}</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button type="button" :class="{ on: !settings.preferSource }" @click="update({ preferSource: null })">{{ t('settings.a29') }}</button>
              <button
                v-for="source in ALL_SOURCES"
                :key="source"
                type="button"
                :class="{ on: settings.preferSource === source }"
                @click="update({ preferSource: source })"
              >
                {{ sourceShort(source) }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>{{ t('settings.a30') }}</h2>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a31') }}</p>
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
          <div class="label"><p class="muted">{{ t('settings.a32') }}</p></div>
          <div class="control wide">
            <code class="preview mono">{{ preview }}.mp3</code>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a33') }}</p>
            <small>{{ t('settings.a34') }}</small>
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
            <p>{{ t('settings.a35') }}</p>
            <small>{{ t('settings.a36') }}</small>
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
            <p>{{ t('settings.a37') }}</p>
            <small>{{ t('settings.a38') }}</small>
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
            <p>{{ t('settings.a39') }}</p>
            <small>{{ t('settings.a40') }}</small>
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
        <h2>{{ t('settings.a41') }}</h2>
        <div class="row">
          <div class="label">
            <p>{{ t('settings.a42') }}</p>
            <small>{{ t('settings.a43') }}</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="option in [
                  { value: 'light', label: 'theme.light' },
                  { value: 'dark', label: 'theme.dark' },
                  { value: 'system', label: 'theme.system' },
                ]"
                :key="option.value"
                type="button"
                :class="{ on: settings.theme === option.value }"
                @click="update({ theme: option.value as Settings['theme'] })"
              >
                {{ t(option.label) }}
              </button>
            </div>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a44') }}</p>
            <small>{{ t('settings.a45') }}</small>
          </div>
          <div class="control accent-row">
            <button
              v-for="accent in ACCENTS"
              :key="accent.id"
              class="accent-dot"
              :class="{ on: settings.accent === accent.id }"
              type="button"
              :title="t(accent.label)"
              :style="{ '--sw': accent.accent }"
              @click="update({ accent: accent.id })"
            >
              <Icon v-if="settings.accent === accent.id" name="check" :size="12" />
            </button>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a46') }}</p>
            <small>{{ t('settings.a47') }}</small>
          </div>
          <div class="control style-row">
            <button
              v-for="item in UI_STYLES"
              :key="item.id"
              class="style-card"
              :class="{ on: store.uiStyle === item.id }"
              type="button"
              :title="t(item.desc)"
              @click="store.setUiStyle(item.id)"
            >
              <span
                class="style-swatch"
                :style="{ '--sw-a': item.preview[0], '--sw-b': item.preview[1] }"
              />
              <span class="style-name">{{ t(item.label) }}</span>
            </button>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>{{ t('settings.a48') }}</h2>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a49') }}</p>
            <small>{{ t('settings.a50') }}</small>
          </div>
          <div class="control">
            <input
              class="range"
              type="range"
              :min="LYRIC_FONT_MIN"
              :max="LYRIC_FONT_MAX"
              step="1"
              :value="lyricStore.fontSize"
              @input="lyricStore.setFontSize(Number(($event.target as HTMLInputElement).value))"
            />
            <span class="value mono">{{ lyricStore.fontSize }}</span>
          </div>
        </div>

        <div class="row lyric-preview-row">
          <div class="label"><p class="muted">{{ t('settings.a32') }}</p></div>
          <div class="control wide">
            <div class="lyric-preview">
              <p class="line" :style="{ fontSize: `${lyricStore.fontSize}px` }">是谁打翻前世柜 惹尘埃是非</p>
              <p
                class="tr"
                :style="{ fontSize: `${Math.round(lyricStore.fontSize * 0.62)}px` }"
              >
                Who knocked over the cupboard of a previous life
              </p>
            </div>
            <button class="btn btn-sm" type="button" @click="lyricStore.setFontSize(22)">{{ t('settings.a51') }}</button>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a52') }}</p>
            <small>{{ t('settings.a53') }}</small>
          </div>
          <div class="control">
            <input
              class="range"
              type="range"
              :min="-3000"
              :max="3000"
              step="100"
              :value="lyricStore.offsetMs"
              @input="lyricStore.setOffset(Number(($event.target as HTMLInputElement).value))"
            />
            <span class="value mono offset-value">{{ formatOffset(lyricStore.offsetMs) }}</span>
            <button class="btn btn-sm" type="button" @click="lyricStore.setOffset(0)">{{ t('settings.a54') }}</button>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a55') }}</p>
            <small>{{ t('settings.a56') }}</small>
          </div>
          <div class="control">
            <button
              class="switch"
              type="button"
              :class="{ on: lyricStore.showInBar }"
              :aria-pressed="lyricStore.showInBar"
              @click="lyricStore.setShowInBar(!lyricStore.showInBar)"
            />
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a57') }}</p>
            <small>{{ t('settings.a58') }}</small>
          </div>
          <div class="control">
            <div class="segmented">
              <button
                v-for="item in LYRIC_MODES"
                :key="item.id"
                type="button"
                :class="{ on: lyricStore.mode === item.id }"
                @click="lyricStore.setMode(item.id)"
              >
                {{ t(item.label) }}
              </button>
            </div>
          </div>
        </div>

        <div class="row">
          <div class="label">
            <p>{{ t('settings.a59') }}</p>
            <small>{{ t('settings.a60') }}</small>
          </div>
          <div class="control style-row">
            <button
              v-for="item in LYRIC_THEMES"
              :key="item.id"
              class="style-card"
              :class="{ on: lyricStore.theme === item.id }"
              type="button"
              :title="t(item.desc)"
              @click="lyricStore.setTheme(item.id)"
            >
              <span
                class="style-swatch"
                :style="{ '--sw-a': item.preview[0], '--sw-b': item.preview[1] }"
              />
              <span class="style-name">{{ t(item.label) }}</span>
            </button>
          </div>
        </div>
      </section>

      <section class="card">
        <h2>{{ t('settings.a61') }}</h2>
        <div v-for="source in ALL_SOURCES" :key="source" class="row">
          <div class="label">
            <p>{{ sourceName(source) }}</p>
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

      <section class="card">
        <h2>{{ t('settings.a62') }}</h2>
        <div class="row">
          <div class="label">
            <p>{{ t('settings.a63') }}</p>
            <small>{{ t('settings.a64') }}</small>
          </div>
          <div class="control">
            <button
              class="switch"
              type="button"
              role="switch"
              :aria-checked="store.debugMode"
              :class="{ on: store.debugMode }"
              @click="store.setDebugMode(!store.debugMode)"
            />
          </div>
        </div>

        <template v-if="store.debugMode">
          <div class="row">
            <div class="label">
              <p>{{ t('settings.a65') }}</p>
              <small>{{ t('settings.a66') }}</small>
            </div>
            <div class="control">
              <button class="btn btn-sm" type="button" @click="onboarding.replay()">
                <Icon name="play" :size="13" />{{ t('settings.a67') }}</button>
            </div>
          </div>

          <div class="row">
            <div class="label">
              <p>{{ t('settings.a68') }}</p>
              <small>{{ t('settings.a69') }}</small>
            </div>
            <div class="control debug-chips">
              <span class="chip mono">{{ t('debug.style', { style: store.uiStyle }) }}</span>
              <span class="chip mono">{{ transport.mode }}</span>
            </div>
          </div>
        </template>
      </section>

      <section class="card about">
        <h2>关于</h2>
        <div class="about-grid">
          <div>
            <p class="k">Melodex Music</p>
            <p class="v muted">v{{ version }} · Apache-2.0</p>
          </div>
          <div>
            <p class="k">运行模式</p>
            <p class="v muted">{{ transport.mode === 'tauri' ? t('about.engine.local') : t('about.engine.demo') }}</p>
          </div>
          <div>
            <p class="k">{{ t('about.license') }}</p>
            <p class="v muted">{{ t('about.licenseNote') }}</p>
          </div>
        </div>

        <p class="capabilities">
          {{ t('about.capabilities') }}
        </p>

        <div class="update-row">
          <div>
            <p class="k">{{ t('settings.update.title') }}</p>
            <p class="v muted">
              <template v-if="updateStore.checking">{{ t('settings.update.checking') }}</template>
              <template v-else-if="updateStore.error">{{ t('settings.update.failed', { err: updateStore.error }) }}</template>
              <template v-else-if="updateStore.info?.available">
                {{ t('settings.update.available', { v: updateStore.info.latest }) }}
              </template>
              <template v-else-if="updateStore.info">{{ t('settings.update.latest') }}</template>
              <template v-else>{{ t('settings.update.idle') }}</template>
            </p>
          </div>
          <button class="btn btn-sm" type="button" :disabled="updateStore.checking" @click="checkUpdate">
            <Icon name="refresh" :size="14" />
            {{ t('settings.update.button') }}
          </button>
        </div>

        <p class="disclaimer">
          {{ t('settings.disclaimer') }}
        </p>
      </section>
    </template>
  </div>

</template>

<style scoped>
.view {
  padding: var(--view-pad);
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

.link-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  text-decoration: none;
  color: inherit;
}
.link-card:hover {
  border-color: color-mix(in srgb, var(--accent) 40%, var(--line));
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
.offset-value {
  width: 48px;
}
/* 限速值要同时容纳「不限」和「1024 KB/s」，18px 会把字挤成竖排 */
.speed-field {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.speed-input {
  width: 78px;
  flex: 0 0 auto;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.speed-unit {
  font-size: 11px;
  color: var(--text-3);
  white-space: nowrap;
}
.proxy-hint {
  margin: 8px 0 0;
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--danger);
}

/* 协议下拉和地址输入挤一行：下拉按内容宽度，地址吃掉剩下的 */
.proxy-line {
  display: flex;
  gap: 8px;
  min-width: 0;
}
.select {
  flex: none;
  height: 32px;
  padding: 0 8px;
  border-radius: var(--r-sm);
  border: 1px solid var(--line);
  background: var(--surface-2);
  color: inherit;
  font-size: 12.5px;
  outline: none;
}
.select:focus {
  border-color: color-mix(in srgb, var(--accent) 50%, transparent);
}
.input:disabled {
  opacity: 0.45;
}
.scope-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 14px;
}
.scope {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text-2);
  cursor: pointer;
  user-select: none;
}
.scope input {
  width: 14px;
  height: 14px;
  margin: 0;
  accent-color: var(--accent);
}

/* 歌词字号预览：直接拿真实字号渲染一句，调完就看见效果 */
.lyric-preview-row {
  align-items: flex-start;
}
.lyric-preview {
  flex: 1;
  min-width: 0;
  padding: 10px 14px;
  border-radius: var(--r-md);
  border: 1px dashed var(--line-strong);
  background: var(--surface-2);
}
.lyric-preview .line {
  margin: 0;
  font-weight: 600;
  line-height: 1.5;
  color: var(--text);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.lyric-preview .tr {
  margin: 4px 0 0;
  line-height: 1.4;
  color: var(--text-3);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chip {
  cursor: pointer;
}

/* 分平台音质：每个平台一张小卡，档位用平台自己的名字 */
.quality-grid {
  display: grid;
  gap: 10px;
  padding: 4px 0 12px;
  border-top: 1px solid var(--line);
}
.platform {
  border: 1px solid var(--line);
  border-radius: var(--r-sm);
  background: var(--surface-2);
  padding: 10px 12px 12px;
}
.platform-head {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 12.5px;
}
.platform-head strong {
  font-weight: 600;
}
.platform-head .link {
  margin-left: auto;
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  font-size: 11px;
  cursor: pointer;
  text-decoration: underline;
}
.platform-head .tiny {
  margin-left: auto;
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}
.note {
  margin: 4px 0 8px;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-3);
}
.tiers {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.tier {
  display: grid;
  gap: 1px;
  padding: 6px 10px;
  border: 1px solid var(--line);
  border-radius: var(--r-sm);
  background: var(--surface);
  text-align: left;
  cursor: pointer;
  transition: all var(--dur) var(--ease);
}
.tier:hover {
  border-color: color-mix(in srgb, var(--accent) 40%, var(--line));
}
.tier.on {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, var(--surface));
}
.tier-name {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--text);
}
.tier.on .tier-name {
  font-weight: 600;
}
.tier-detail {
  font-size: 10.5px;
  color: var(--text-3);
}
.vip {
  padding: 0 4px;
  border-radius: 3px;
  font-size: 9.5px;
  line-height: 1.5;
  color: #b4802a;
  background: color-mix(in srgb, #e8a33d 18%, transparent);
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

.accent-row {
  gap: 8px;
}
.accent-dot {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  color: #fff;
  background: var(--sw);
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
  transition: transform var(--dur) var(--ease), box-shadow var(--dur) var(--ease);
}
.accent-dot:hover {
  transform: scale(1.12);
}
.accent-dot.on {
  box-shadow: 0 0 0 2px var(--surface), 0 0 0 4px var(--sw);
}

/* 界面风格：用一小块渐变预览代替纯文字，选之前就能看出质感差别 */
.style-row {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
.style-card {
  display: grid;
  gap: 6px;
  justify-items: center;
  padding: 7px 7px 6px;
  border-radius: var(--r-md);
  border: 1px solid var(--line);
  background: var(--surface-2);
  transition: border-color var(--dur) var(--ease), background var(--dur) var(--ease);
}
.style-card:hover {
  border-color: var(--line-strong);
}
.style-card.on {
  border-color: var(--accent);
  background: var(--active);
}
.style-swatch {
  display: block;
  width: 58px;
  height: 34px;
  border-radius: calc(var(--r-sm) - 2px);
  background: linear-gradient(140deg, var(--sw-a), var(--sw-b));
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
}
.style-name {
  font-size: 11.5px;
  color: var(--text-2);
}
.style-card.on .style-name {
  color: var(--text);
  font-weight: 500;
}

.debug-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
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
.capabilities {
  margin: 14px 0 0;
  padding: 10px 12px;
  border-radius: var(--r-sm);
  background: var(--surface-2);
  border: 1px solid var(--line);
  font-size: 11.5px;
  line-height: 1.75;
  color: var(--text-2);
}

@media (max-width: 860px) {
  .view {
    padding: calc(env(safe-area-inset-top, 0px) + 18px) 14px
      calc(env(safe-area-inset-bottom, 0px) + 30px);
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
