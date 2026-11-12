<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'

import Icon from '@/components/Icon.vue'
import { formatDuration } from '@/lib/format'
import { useI18n } from '@/lib/i18n'
import {
  APPLE_STORES,
  PLATFORM_CAPABILITIES,
  searchAppleMusic,
  type AppleStore,
  type AppleTrack,
  type PlatformCapability,
} from '@/lib/streaming'

const { t } = useI18n()

const keyword = ref('')
const store = ref<AppleStore>('us')
const results = ref<AppleTrack[]>([])
const loading = ref(false)
const error = ref('')
const searched = ref(false)

/** 试听：同一个 audio 元素，切歌时先停再换 src，避免两首叠着响 */
const previewing = ref<number | null>(null)
let audio: HTMLAudioElement | null = null

function player(): HTMLAudioElement {
  if (!audio) {
    audio = new Audio()
    audio.addEventListener('ended', () => (previewing.value = null))
  }
  return audio
}

function togglePreview(track: AppleTrack) {
  if (!track.previewUrl) return
  const el = player()
  if (previewing.value === track.id) {
    el.pause()
    previewing.value = null
    return
  }
  el.pause()
  el.src = track.previewUrl
  void el.play()
  previewing.value = track.id
}

async function submit() {
  const term = keyword.value.trim()
  if (!term || loading.value) return
  loading.value = true
  error.value = ''
  try {
    results.value = await searchAppleMusic(term, store.value)
    searched.value = true
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
    results.value = []
  } finally {
    loading.value = false
  }
}

function switchStore(next: AppleStore) {
  if (store.value === next) return
  store.value = next
  if (searched.value) void submit()
}

const others = computed<PlatformCapability[]>(() =>
  PLATFORM_CAPABILITIES.filter((item) => item.id !== 'apple'),
)

const tierLabel = computed<Record<string, string>>(() => ({
  ready: t('platforms.tier.ready'),
  credentials: t('platforms.tier.credentials'),
  blocked: t('platforms.tier.blocked'),
}))

function searchLink(item: PlatformCapability): string {
  const template = item.searchUrl
  if (!template) return item.home
  return template.replace('{q}', encodeURIComponent(keyword.value.trim() || 'music'))
}

onUnmounted(() => {
  audio?.pause()
  audio = null
})
</script>

<template>
  <div class="view">
    <header class="head">
      <div class="head-text">
        <h1>{{ t('platforms.title') }}</h1>
        <p class="muted">{{ t('platforms.subtitle') }}</p>
      </div>
    </header>

    <!-- Apple Music：能搜、能试听 30 秒、不能放全曲 -->
    <section class="card">
      <div class="card-head">
        <div>
          <h2>{{ t('platforms.apple.title') }}</h2>
          <p class="muted">{{ t('platforms.apple.desc') }}</p>
        </div>
        <span class="badge ok">{{ tierLabel.ready }}</span>
      </div>

      <div class="searchbar">
        <Icon name="search" :size="16" />
        <input
          v-model="keyword"
          type="search"
          :placeholder="t('platforms.apple.placeholder')"
          autocomplete="off"
          @keydown.enter="submit"
        />
        <button class="btn btn-primary btn-sm" type="button" :disabled="loading || !keyword.trim()" @click="submit">
          {{ loading ? t('common.loading') : t('platforms.search') }}
        </button>
      </div>

      <div class="stores">
        <button
          v-for="item in APPLE_STORES"
          :key="item.id"
          type="button"
          class="chip chip-btn"
          :class="{ on: store === item.id }"
          @click="switchStore(item.id)"
        >
          {{ item.label }}
        </button>
      </div>

      <p v-if="error" class="err">{{ error }}</p>

      <ul v-if="results.length" class="apple-list">
        <li v-for="track in results" :key="track.id" class="apple-row">
          <img v-if="track.artwork" :src="track.artwork" alt="" loading="lazy" class="art" />
          <div v-else class="art placeholder" />
          <div class="meta">
            <p class="title">{{ track.title }}</p>
            <p class="sub">{{ track.artist }}<template v-if="track.album"> · {{ track.album }}</template></p>
          </div>
          <span class="mono dur">{{ formatDuration(track.durationMs) }}</span>
          <button
            class="icon-btn"
            type="button"
            :disabled="!track.previewUrl"
            :title="track.previewUrl ? t('platforms.preview') : t('platforms.noPreview')"
            @click="togglePreview(track)"
          >
            <Icon :name="previewing === track.id ? 'pause' : 'play'" :size="14" />
          </button>
          <a class="icon-btn" :href="track.url" target="_blank" rel="noreferrer" :title="t('platforms.openIn')">
            <Icon name="external" :size="14" />
          </a>
        </li>
      </ul>

      <p v-else-if="searched && !loading" class="muted empty">{{ t('platforms.apple.empty') }}</p>
    </section>

    <!-- 其余平台：只说明边界，不给假按钮 -->
    <section class="card">
      <div class="card-head">
        <div>
          <h2>{{ t('platforms.others.title') }}</h2>
          <p class="muted">{{ t('platforms.others.desc') }}</p>
        </div>
      </div>
      <ul class="cap-list">
        <li v-for="item in others" :key="item.id" class="cap-row">
          <div class="cap-text">
            <p class="title">
              {{ t(`platforms.${item.id}.title`) }}
              <span class="badge" :class="item.tier">{{ tierLabel[item.tier] }}</span>
            </p>
            <p class="sub muted">{{ t(`platforms.${item.id}.desc`) }}</p>
          </div>
          <a class="btn btn-sm" :href="searchLink(item)" target="_blank" rel="noreferrer">
            {{ item.searchUrl ? t('platforms.openSearch') : t('platforms.openHome') }}
          </a>
        </li>
      </ul>
    </section>

    <p class="muted footnote">{{ t('platforms.footnote') }}</p>
  </div>
</template>

<style scoped>
.head {
  margin-bottom: 20px;
}
h1 {
  margin: 0 0 6px;
  font-size: 26px;
  letter-spacing: -0.02em;
}
.muted {
  color: var(--text-2);
  margin: 0;
  font-size: 13px;
}
.card {
  padding: 18px 20px;
  border-radius: var(--r-lg);
  background: var(--surface);
  border: 1px solid var(--line);
  margin-bottom: 16px;
}
.card-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}
.card-head h2 {
  margin: 0 0 4px;
  font-size: 16px;
}
.badge {
  flex: none;
  padding: 3px 9px;
  border-radius: 999px;
  font-size: 11px;
  border: 1px solid var(--line);
  color: var(--text-2);
  white-space: nowrap;
}
.badge.ok {
  color: var(--ok, #0f9d58);
  border-color: color-mix(in srgb, var(--ok, #0f9d58) 40%, transparent);
}
.badge.credentials {
  color: var(--warning, #b7791f);
  border-color: color-mix(in srgb, var(--warning, #b7791f) 40%, transparent);
}
.badge.blocked {
  color: var(--text-3);
}
.searchbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px 0 12px;
  height: 40px;
  border-radius: 999px;
  background: var(--surface-2);
  border: 1px solid var(--line);
}
.searchbar input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: none;
  outline: none;
  font: inherit;
  color: inherit;
}
.stores {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 12px 0 4px;
}
.chip.on {
  border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  color: var(--accent);
}
.apple-list {
  list-style: none;
  margin: 12px 0 0;
  padding: 0;
}
.apple-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 0;
  border-bottom: 1px solid var(--line);
}
.apple-row:last-child {
  border-bottom: 0;
}
.art {
  width: 44px;
  height: 44px;
  border-radius: 8px;
  object-fit: cover;
  flex: none;
}
.art.placeholder {
  background: var(--surface-2);
}
.meta {
  flex: 1;
  min-width: 0;
}
.meta .title {
  margin: 0;
  font-size: 13.5px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.meta .sub {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dur {
  font-size: 12px;
  color: var(--text-3);
  flex: none;
}
.cap-list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.cap-row {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 0;
  border-bottom: 1px solid var(--line);
}
.cap-row:last-child {
  border-bottom: 0;
}
.cap-text {
  flex: 1;
  min-width: 0;
}
.cap-text .title {
  margin: 0 0 3px;
  font-size: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.cap-text .sub {
  margin: 0;
  font-size: 12px;
  line-height: 1.55;
}
.empty {
  margin-top: 14px;
}
.err {
  margin: 12px 0 0;
  font-size: 12px;
  color: var(--danger);
}
.footnote {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.6;
}
@media (max-width: 600px) {
  .apple-row .dur {
    display: none;
  }
  .cap-row {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
