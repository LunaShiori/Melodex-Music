<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'

import { usePlayerStore } from '@/stores/player'
import Icon from '@/components/Icon.vue'
import { formatDuration } from '@/lib/format'
import { useI18n } from '@/lib/i18n'
import type { BiliVideo } from '@/lib/transport/types'
import { transport } from '@/lib/transport'
import {
  APPLE_STORES,
  PLATFORM_CAPABILITIES,
  searchAppleMusic,
  type AppleStore,
  type AppleTrack,
  type PlatformCapability,
} from '@/lib/streaming'

const props = defineProps<{ query?: string; embedded?: boolean }>()
const { t } = useI18n()

type Discovery = { url: string; title: string; by: string; platform: string }
const shelfKey = 'melodex.discovery.v1'
function readShelf(): Discovery[] {
  try {
    const rows = JSON.parse(localStorage.getItem(shelfKey) || '[]')
    return Array.isArray(rows) ? rows.filter(row => typeof row?.url === 'string' && /^https:\/\//i.test(row.url) && typeof row.title === 'string' && typeof row.by === 'string') : []
  } catch { return [] }
}
const shelf = ref(readShelf())
const picked=ref<string[]>([])
function matchPicked(){
  const terms=shelf.value.filter(r=>picked.value.includes(r.url)).map(r=>r.platform==='Apple Music'?r.title+' '+r.by:r.title)
  sessionStorage.setItem('melodex.matchDraft',terms.join('\n'))
}
const filter = ref('')
const filteredShelf = computed(() => shelf.value.filter(row => [row.title,row.by,row.platform].join(' ').toLowerCase().includes(filter.value.trim().toLowerCase())))
const savedUrls = computed(() => new Set(shelf.value.map(row => row.url)))
const shelfError = ref(false)
function toggleSaved(row: Discovery) {
  const updated = savedUrls.value.has(row.url) ? shelf.value.filter(item => item.url !== row.url) : [row, ...shelf.value]
  try {
    localStorage.setItem(shelfKey, JSON.stringify(updated))
    shelf.value = updated
    shelfError.value = false
  } catch { shelfError.value = true }
}
const keyword = ref('')
const store = ref<AppleStore>('us')
const results = ref<AppleTrack[]>([])
const loading = ref(false)
const error = ref('')
const searched = ref(false)
let appleRequest = 0
let biliRequest = 0
let previewRequest = 0

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

async function togglePreview(track: AppleTrack) {
  if (!track.previewUrl) return
  const request = ++previewRequest
  const el = player()
  if (previewing.value === track.id) {
    el.pause()
    previewing.value = null
    return
  }
  el.pause()
  el.src = track.previewUrl
  const main = usePlayerStore()
  if (main.playing) main.toggle()
  try { await el.play(); if (request === previewRequest) previewing.value = track.id }
  catch { if (request === previewRequest) { error.value = t('platforms.previewFailed'); previewing.value = null } }
}

async function submit() {
  const term = keyword.value.trim()
  if (!term) return
  previewRequest++; audio?.pause(); previewing.value = null
  const request = ++appleRequest
  loading.value = true
  error.value = ''
  try {
    const rows = await searchAppleMusic(term, store.value)
    if (request !== appleRequest) return
    results.value = rows
    searched.value = true
  } catch (err) {
    if (request !== appleRequest) return
    error.value = err instanceof Error ? err.message : String(err)
    results.value = []
  } finally {
    if (request === appleRequest) loading.value = false
  }
}

function switchStore(next: AppleStore) {
  if (store.value === next) return
  store.value = next
  if (searched.value) void submit()
}

// B 站音乐区：只搜不播。接口要 buvid3 Cookie 且没有 CORS 头，
// 所以走桌面壳的 Rust 命令；浏览器里只能给样例数据。
const biliKeyword = ref('')
const biliResults = ref<BiliVideo[]>([])
const biliLoading = ref(false)
const biliError = ref('')
const biliSearched = ref(false)
const biliMore = ref(false)
let biliPage = 0, biliTerm = ''

async function submitBili(append = false) {
  if (append && (biliLoading.value || !biliMore.value)) return
  const term = append ? biliTerm : biliKeyword.value.trim()
  if (!term) return
  const page = append ? biliPage + 1 : 1
  if (!append) { biliMore.value = false; biliResults.value = []; biliTerm = term }
  const request = ++biliRequest
  biliLoading.value = true
  biliError.value = ''
  try {
    const rows = await transport.searchBilibili(term, page)
    if (request !== biliRequest) return
    const seen = new Set(biliResults.value.map(v => v.bvid))
    const fresh = rows.filter(v => !seen.has(v.bvid))
    biliResults.value = append ? [...biliResults.value, ...fresh] : rows
    biliPage = page
    biliMore.value = rows.length >= 20 && (!append || fresh.length > 0)
    biliSearched.value = true
  } catch (err) {
    if (request !== biliRequest) return
    biliError.value = err instanceof Error ? err.message : String(err)
    if (!append) biliResults.value = []
  } finally {
    if (request === biliRequest) biliLoading.value = false
  }
}

/** 播放数太长会顶到时长列，按万/亿折一下 */
function countOf(n: number): string {
  if (n >= 100_000_000) return `${(n / 100_000_000).toFixed(1)} 亿`
  if (n >= 10_000) return `${(n / 10_000).toFixed(1)} 万`
  return String(n)
}

const others = computed<PlatformCapability[]>(() =>
  PLATFORM_CAPABILITIES.filter((item) => item.id !== 'apple' && item.id !== 'bilibili'),
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

function searchAll() {
  biliKeyword.value = keyword.value
  void submit()
  void submitBili()
}
watch(() => props.query, q => {
  if (q?.trim()) { keyword.value = q; searchAll() }
}, { immediate: true })
async function openExternal(url: string) {
  try { await transport.openUrl(url) } catch (err) { error.value = String(err) }
}
watch(() => usePlayerStore().playing, playing => {
  if (playing) { previewRequest++; audio?.pause(); previewing.value = null }
})
onUnmounted(() => {
  previewRequest += 1
  appleRequest += 1
  biliRequest += 1
  audio?.pause()
  audio = null
})
</script>

<template>
  <div class="view" :class="{ embedded }">
    <header class="head">
      <div class="head-text">
        <h1>{{ t('platforms.title') }} <span class="beta-tag">BETA</span></h1>
        <p class="muted">{{ t('platforms.subtitle') }}</p>
      </div>
    </header>

    <details class="card discovery" v-if="shelf.length || shelfError">
      <summary>{{ t('discovery.title') }} · {{ shelf.length }}</summary>
      <input type="search" v-model="filter" :placeholder="t('discovery.filter')" :aria-label="t('discovery.filter')" />
      <p v-if="!filteredShelf.length">{{ t('discovery.empty') }}</p>
      <RouterLink v-if="picked.length" class="btn" to="/favorites?match=1" @click="matchPicked">批量匹配到歌单 · {{picked.length}}</RouterLink>
      <article v-for="row in filteredShelf" :key="row.url">
        <input type="checkbox" v-model="picked" :value="row.url" :aria-label="row.title"/><div class="meta"><strong>{{ row.title }}</strong><p class="sub">{{ row.platform }} · {{ row.by }}</p></div>
        <button class="btn btn-sm" @click="openExternal(row.url)">{{ t('platforms.openIn') }}</button>
        <RouterLink class="btn btn-sm" :to="{path:'/search',query:{q:row.platform === 'Apple Music' ? row.title+' '+row.by : row.title}}">{{ t('discovery.match') }}</RouterLink>
        <button class="btn btn-sm" @click="toggleSaved(row)">{{ t('discovery.remove') }}</button>
      </article>
    </details>
    <p v-if="shelfError" role="alert">{{ t('discovery.error') }}</p>
    <form class="unified searchbar" @submit.prevent="searchAll">
      <input v-model="keyword" type="search" :aria-label="t('platforms.unified')" :placeholder="t('platforms.unified')" />
      <button class="btn btn-primary btn-sm" :disabled="!keyword.trim()">{{ t('platforms.searchAll') }}</button>
    </form>
    <p class="muted beta-note">{{ t('platforms.betaNote') }}</p>
    <!-- Apple Music：能搜、能试听 30 秒、不能放全曲 -->
    <div class="platform-pair">
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
            <button class="chip chip-btn" :aria-pressed="savedUrls.has(track.url)" @click="toggleSaved({ url:track.url, title:track.title, by:track.artist, platform:'Apple Music' })">{{ t(savedUrls.has(track.url) ? 'discovery.saved' : 'discovery.save') }}</button>
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
          <a class="icon-btn" :href="track.url" @click.prevent="openExternal(track.url)" target="_blank" rel="noreferrer" :title="t('platforms.openIn')">
            <Icon name="external" :size="14" />
          </a>
        </li>
      </ul>

      <p v-else-if="searched && !loading" class="muted empty">{{ t('platforms.apple.empty') }}</p>
    </section>

    <!-- B 站音乐区：能搜到视频，播放交给 B 站 -->
    <section class="card">
      <div class="card-head">
        <div>
          <h2>{{ t('platforms.bilibili.title') }}</h2>
          <p class="muted">{{ t('platforms.bilibili.searchDesc') }}</p>
        </div>
        <span class="badge ok">{{ t('platforms.bilibili.badge') }}</span>
      </div>

      <div class="searchbar">
        <Icon name="search" :size="16" />
        <input
          v-model="biliKeyword"
          type="search"
          :placeholder="t('platforms.bilibili.placeholder')"
          autocomplete="off"
          @keydown.enter="submitBili()"
        />
        <button
          class="btn btn-primary btn-sm"
          type="button"
          :disabled="biliLoading || !biliKeyword.trim()"
          @click="submitBili()"
        >
          {{ biliLoading ? t('common.loading') : t('platforms.search') }}
        </button>
      </div>

      <p v-if="biliError" class="err" role="alert">{{ biliError }}</p>

      <ul v-if="biliResults.length" class="apple-list">
        <li v-for="video in biliResults" :key="video.bvid" class="apple-row">
          <img v-if="video.cover" :src="video.cover" alt="" loading="lazy" class="art" />
          <div v-else class="art placeholder" />
          <div class="meta">
            <p class="title">{{ video.title }}</p>
            <p class="sub">{{ video.author }} · {{ t('platforms.bilibili.plays', { n: countOf(video.plays) }) }}</p>
            <button class="chip chip-btn" :aria-pressed="savedUrls.has(video.url)" @click="toggleSaved({ url:video.url, title:video.title, by:video.author, platform:'Bilibili' })">{{ t(savedUrls.has(video.url) ? 'discovery.saved' : 'discovery.save') }}</button>
          </div>
          <span class="mono dur">{{ formatDuration(video.durationSecs * 1000) }}</span>
          <a class="icon-btn" :href="video.url" @click.prevent="openExternal(video.url)" target="_blank" rel="noreferrer" :title="t('platforms.openIn')">
            <Icon name="external" :size="14" />
          </a>
        </li>
      </ul>

      <p v-else-if="biliSearched && !biliLoading" class="muted empty">{{ t('platforms.bilibili.empty') }}</p>
      <p v-else class="muted empty">{{ t('platforms.bilibili.hint') }}</p>
      <button v-if="biliMore" class="btn btn-sm" :disabled="biliLoading" @click="submitBili(true)">{{ t(biliLoading ? 'common.loading' : 'platforms.moreResults') }}</button>
    </section>

    </div>
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
          <a class="btn btn-sm" :href="searchLink(item)" @click.prevent="openExternal(searchLink(item))" target="_blank" rel="noreferrer">
            {{ item.searchUrl ? t('platforms.openSearch') : t('platforms.openHome') }}
          </a>
        </li>
      </ul>
    </section>

    <p class="muted footnote">{{ t('platforms.footnote') }}</p>
  </div>
</template>

<style scoped>
.discovery summary{cursor:pointer;font-weight:650}.discovery input{margin:12px 0;padding:8px;max-width:100%}.discovery article{display:flex;align-items:center;flex-wrap:wrap;gap:8px;padding:10px 0;border-top:1px solid var(--line)}.discovery article .meta{flex:1 1 180px;overflow-wrap:anywhere}

.view { padding: var(--view-pad); max-width: 1160px; margin: auto; min-width: 0; }
.view.embedded { padding: 0; margin-top: 24px; }
.unified { margin-bottom: 12px; }
.beta-note { margin-bottom: 20px !important; line-height: 1.6; }
.card-head > div { min-width: 0; }
.card-head, .cap-text .title { flex-wrap: wrap; }
@media(max-width: 600px) { .card { padding: 14px !important; } .apple-row { gap: 7px !important; } }
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
