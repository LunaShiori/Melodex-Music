<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'

import Icon from '@/components/Icon.vue'
import TrackRow from '@/components/TrackRow.vue'
import { coverOf } from '@/lib/artwork'
import { onCoverError } from '@/lib/cover'
import { transport } from '@/lib/transport'
import { SOURCE_META } from '@/lib/types'
import type {
  PlaylistDetail,
  PlaylistSummary,
  SourceId,
  Track,
  Variant,
} from '@/lib/types'
import { useAccountsStore } from '@/stores/accounts'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'

const accounts = useAccountsStore()
const tasks = useTasksStore()
const player = usePlayerStore()

const loading = ref(false)
const error = ref('')
const playlists = ref<PlaylistSummary[]>([])
const filter = ref<'all' | SourceId>('all')

/** 当前打开的歌单（null = 列表模式） */
const opened = ref<PlaylistDetail | null>(null)
const opening = ref(false)

/** 只保留支持账号登录的平台，酷我不提供账号歌单 */
const ACCOUNT_SOURCES: SourceId[] = ['netease', 'qq']

/* ---------- 列表：歌单名搜索 + 显示方式 ---------- */

type PlaylistView = 'grid' | 'compact' | 'list'

const VIEWS: { id: PlaylistView; label: string; icon: string }[] = [
  { id: 'grid', label: '大封面', icon: 'grid' },
  { id: 'compact', label: '小图标', icon: 'compact' },
  { id: 'list', label: '列表', icon: 'rowlist' },
]

const VIEW_KEY = 'melodex.playlists.view'

function readView(): PlaylistView {
  try {
    const stored = localStorage.getItem(VIEW_KEY)
    if (stored === 'grid' || stored === 'compact' || stored === 'list') return stored
  } catch {
    // 存储被禁就用默认值
  }
  return 'grid'
}

const view = ref<PlaylistView>(readView())

function setView(next: PlaylistView) {
  view.value = next
  try {
    localStorage.setItem(VIEW_KEY, next)
  } catch {
    // 只在本次会话生效
  }
}

/** 歌单列表的搜索词（按歌单名 / 创建者过滤） */
const query = ref('')

const groups = computed(() => {
  const wanted = filter.value === 'all' ? ACCOUNT_SOURCES : [filter.value]
  const key = query.value.trim().toLowerCase()
  return wanted.map((source) => {
    const all = playlists.value.filter((item) => item.source === source)
    const items = key
      ? all.filter((item) =>
          `${item.name} ${item.creator ?? ''}`.toLowerCase().includes(key),
        )
      : all
    return {
      source,
      label: SOURCE_META[source].label,
      loggedIn: Boolean(accounts.accountOf(source)),
      items,
      total: all.length,
    }
  })
})

const total = computed(() => playlists.value.length)
const matchedTotal = computed(() => groups.value.reduce((sum, group) => sum + group.items.length, 0))
/** 搜了但一条都没有——和「根本没读到歌单」要分开提示 */
const searching = computed(() => query.value.trim().length > 0 && matchedTotal.value === 0)

/* ---------- 详情：歌单内搜索 + 分类筛选 ---------- */

type FeeFilter = 'all' | 'free' | 'vip' | 'lossless'

const FEE_FILTERS: { id: FeeFilter; label: string }[] = [
  { id: 'all', label: '全部' },
  { id: 'free', label: '免费' },
  { id: 'vip', label: '会员' },
  { id: 'lossless', label: '无损' },
]

const trackQuery = ref('')
const feeFilter = ref<FeeFilter>('all')
/** 大歌单一次渲染上千行会卡，分批放开 */
const PAGE = 80
const limit = ref(PAGE)

function toTrack(variant: Variant): Track {
  return {
    fingerprint: `${variant.source}-${variant.id}`,
    title: variant.title,
    artists: variant.artists,
    album: variant.album,
    durationMs: variant.durationMs,
    cover: variant.cover,
    bitrate: variant.bitrate,
    format: variant.format,
    fee: variant.fee,
    variants: [variant],
  }
}

const allTracks = computed(() => (opened.value?.items ?? []).map(toTrack))

function isLossless(track: Track): boolean {
  return track.variants.some(
    (v) => v.format === 'flac' || v.format === 'ape' || v.bitrate >= 900,
  )
}

const filteredTracks = computed(() => {
  const key = trackQuery.value.trim().toLowerCase()
  return allTracks.value.filter((track) => {
    if (key) {
      const haystack = `${track.title} ${track.artists.join(' ')} ${track.album}`.toLowerCase()
      if (!haystack.includes(key)) return false
    }
    switch (feeFilter.value) {
      case 'free':
        return track.fee === 'free'
      case 'vip':
        return track.fee === 'vip' || track.fee === 'paid'
      case 'lossless':
        return isLossless(track)
      default:
        return true
    }
  })
})

/** 真正渲染的那一段 */
const shownTracks = computed(() => filteredTracks.value.slice(0, limit.value))
const hasMore = computed(() => filteredTracks.value.length > shownTracks.value.length)

// 换歌单 / 改条件时回到第一批
watch([trackQuery, feeFilter, opened], () => {
  limit.value = PAGE
})

async function load() {
  loading.value = true
  error.value = ''
  try {
    playlists.value = await transport.myPlaylists(ACCOUNT_SOURCES)
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    loading.value = false
  }
}

async function open(item: PlaylistSummary) {
  opening.value = true
  error.value = ''
  try {
    const detail = await transport.playlist({ source: item.source, id: item.id })
    if (!detail) {
      error.value = '这个歌单没有取到内容，可能已被删除或需要重新登录'
      return
    }
    opened.value = detail
    trackQuery.value = ''
    feeFilter.value = 'all'
    limit.value = PAGE
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err)
  } finally {
    opening.value = false
  }
}

function back() {
  opened.value = null
}

function downloadAll() {
  if (!filteredTracks.value.length) return
  void tasks.enqueue(filteredTracks.value)
}

function downloadOne(track: Track) {
  void tasks.enqueue([track])
}

function playTrack(track: Track) {
  const list = filteredTracks.value
  const index = list.findIndex((item) => item.fingerprint === track.fingerprint)
  void player.playList(list, index < 0 ? 0 : index)
}

/* ---------- 歌单导出 / 批量导入 ---------- */

const exporting = ref(false)
const importing = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

/** 导出文件的结构版本：字段变了就 +1，导入时按它判断兼容性 */
const EXPORT_VERSION = 1

/** 导出的是完整 Variant，导入回来能直接入队，不需要再联网搜一遍 */
function exportPayload(details: PlaylistDetail[]): string {
  return JSON.stringify(
    {
      app: 'melodex',
      kind: 'playlist-export',
      version: EXPORT_VERSION,
      exportedAt: new Date().toISOString(),
      playlists: details.map((detail) => ({
        name: detail.name,
        source: detail.source,
        id: detail.id,
        total: detail.total,
        items: detail.items,
      })),
    },
    null,
    2,
  )
}

function exportFileName(base: string): string {
  const stamp = new Date().toISOString().slice(0, 10)
  return `${base}-${stamp}.json`
}

async function exportOpened() {
  const detail = opened.value
  if (!detail || exporting.value) return
  exporting.value = true
  try {
    const path = await transport.saveTextFile(
      exportFileName(`melodex-歌单-${detail.name || detail.id}`),
      exportPayload([detail]),
    )
    tasks.pushNotice('success', path ? `已导出到 ${path}` : '已开始下载导出文件')
  } catch (err) {
    tasks.pushNotice('error', err instanceof Error ? err.message : String(err))
  } finally {
    exporting.value = false
  }
}

async function exportAll() {
  if (!playlists.value.length || exporting.value) return
  exporting.value = true
  try {
    const details: PlaylistDetail[] = []
    for (const item of playlists.value) {
      const detail = await transport.playlist({ source: item.source, id: item.id })
      if (detail) details.push(detail)
    }
    const path = await transport.saveTextFile(
      exportFileName('melodex-歌单合集'),
      exportPayload(details),
    )
    tasks.pushNotice(
      'success',
      path ? `已导出 ${details.length} 个歌单到 ${path}` : `已开始下载 ${details.length} 个歌单`,
    )
  } catch (err) {
    tasks.pushNotice('error', err instanceof Error ? err.message : String(err))
  } finally {
    exporting.value = false
  }
}

/** 从导出文件还原曲目；按「音源 + id」去重，导入两遍不会重复下载 */
function tracksFromExport(raw: unknown): Track[] {
  const file = raw as { kind?: string; playlists?: Array<{ items?: Variant[] }> } | null
  if (!file || file.kind !== 'playlist-export' || !Array.isArray(file.playlists)) {
    throw new Error('这不是 Melodex 导出的歌单文件')
  }
  const out: Track[] = []
  const seen = new Set<string>()
  for (const playlist of file.playlists) {
    for (const variant of playlist.items ?? []) {
      if (!variant?.source || !variant?.id || !variant?.title) continue
      const key = `${variant.source}-${variant.id}`
      if (seen.has(key)) continue
      seen.add(key)
      out.push(toTrack(variant))
    }
  }
  return out
}

async function onImportFile(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  // 清空 value：同一个文件连选两次也要能再次触发 change
  input.value = ''
  if (!file) return
  importing.value = true
  try {
    const tracks = tracksFromExport(JSON.parse(await file.text()))
    if (!tracks.length) {
      tasks.pushNotice('warning', '这个文件里没有可以下载的曲目')
      return
    }
    await tasks.enqueue(tracks)
    tasks.pushNotice('success', `已导入 ${tracks.length} 首，开始下载`)
  } catch (err) {
    tasks.pushNotice('error', `导入失败：${err instanceof Error ? err.message : String(err)}`)
  } finally {
    importing.value = false
  }
}

onMounted(async () => {
  // 账号状态不随应用启动加载（只有设置页会拉），这里补一次，
  // 否则「未登录」的提示会对着已经登录的账号闪一下
  await Promise.all([accounts.refresh(), load()])
})
</script>

<template>
  <div class="view">
    <header class="head">
      <div>
        <h1>{{ opened ? opened.name : '我的歌单' }}</h1>
        <p class="muted">
          <template v-if="opened">
            来自 {{ SOURCE_META[opened.source].label }} · 共 {{ opened.items.length }} 首
            <template v-if="filteredTracks.length !== opened.items.length">
              · 筛出 {{ filteredTracks.length }} 首
            </template>
          </template>
          <template v-else>
            读取各平台当前登录账号的歌单，点进去就能直接下载
          </template>
        </p>
      </div>

      <div class="head-actions">
        <template v-if="opened">
          <button class="btn" type="button" @click="back">
            <Icon name="chevron" :size="14" class="flip" />
            返回
          </button>
          <button class="btn" type="button" :disabled="exporting" @click="exportOpened">
            <Icon name="download" :size="14" />
            导出
          </button>
          <button
            class="btn btn-primary"
            type="button"
            :disabled="!filteredTracks.length"
            @click="downloadAll"
          >
            <Icon name="download" :size="14" />
            下载{{ feeFilter === 'all' && !trackQuery.trim() ? '全部' : '筛选结果' }}
          </button>
        </template>
        <template v-else>
          <div class="segmented">
            <button
              v-for="option in [
                { value: 'all' as const, label: '全部' },
                { value: 'netease' as const, label: '网易云' },
                { value: 'qq' as const, label: 'QQ 音乐' },
              ]"
              :key="option.value"
              type="button"
              :class="{ on: filter === option.value }"
              @click="filter = option.value"
            >
              {{ option.label }}
            </button>
          </div>
          <div class="view-switch" role="group" aria-label="显示方式">
            <button
              v-for="mode in VIEWS"
              :key="mode.id"
              type="button"
              :class="{ on: view === mode.id }"
              :title="mode.label"
              @click="setView(mode.id)"
            >
              <Icon :name="mode.icon" :size="14" />
            </button>
          </div>
          <button class="icon-btn" type="button" title="刷新" @click="load">
            <Icon name="refresh" :size="15" :class="{ spin: loading }" />
          </button>
          <button
            class="icon-btn"
            type="button"
            title="导出全部歌单"
            :disabled="exporting || !total"
            @click="exportAll"
          >
            <Icon name="download" :size="15" />
          </button>
          <button
            class="icon-btn"
            type="button"
            title="从导出文件导入并批量下载"
            :disabled="importing"
            @click="fileInput?.click()"
          >
            <Icon name="folder" :size="15" />
          </button>
          <input
            ref="fileInput"
            type="file"
            accept=".json,application/json"
            hidden
            @change="onImportFile"
          />
        </template>
      </div>
    </header>

    <p v-if="error" class="alert">
      <Icon name="alert" :size="14" />
      {{ error }}
    </p>

    <!-- 歌单详情 -->
    <template v-if="opened">
      <div class="detail-bar">
        <div class="field">
          <Icon name="search" :size="15" />
          <input v-model="trackQuery" type="search" placeholder="在这个歌单里搜歌名、歌手或专辑" />
          <button v-if="trackQuery" type="button" class="clear" @click="trackQuery = ''">
            <Icon name="close" :size="13" />
          </button>
        </div>
        <div class="chips">
          <button
            v-for="chip in FEE_FILTERS"
            :key="chip.id"
            type="button"
            class="chip chip-btn"
            :class="{ 'is-on': feeFilter === chip.id }"
            @click="feeFilter = chip.id"
          >
            {{ chip.label }}
          </button>
        </div>
      </div>

      <section v-if="shownTracks.length" class="list">
        <TrackRow
          v-for="(track, index) in shownTracks"
          :key="track.fingerprint"
          :track="track"
          :index="index"
          :active="player.current?.fingerprint === track.fingerprint"
          :playing="player.playing"
          @play="playTrack"
          @download="downloadOne"
        />
        <div class="more">
          <button v-if="hasMore" class="btn" type="button" @click="limit += PAGE">
            再显示 {{ Math.min(PAGE, filteredTracks.length - shownTracks.length) }} 首（剩
            {{ filteredTracks.length - shownTracks.length }}）
          </button>
          <p v-else class="muted end">已到末尾</p>
        </div>
      </section>
      <section v-else class="empty">
        <div class="empty-art"><Icon name="music" :size="26" /></div>
        <h3>{{ allTracks.length ? '没有符合条件的歌' : '这个歌单是空的' }}</h3>
        <p class="muted">
          {{
            allTracks.length
              ? '换个关键词，或把筛选切回「全部」。'
              : '换一个歌单试试，或者去搜索页找歌。'
          }}
        </p>
      </section>
    </template>

    <!-- 歌单列表 -->
    <template v-else>
      <div class="list-bar">
        <div class="field">
          <Icon name="search" :size="15" />
          <input v-model="query" type="search" placeholder="搜索歌单名" />
          <button v-if="query" type="button" class="clear" @click="query = ''">
            <Icon name="close" :size="13" />
          </button>
        </div>
        <span class="muted small">
          {{ query.trim() ? `匹配 ${matchedTotal} / ${total} 个` : `${total} 个歌单` }}
        </span>
      </div>

      <section v-for="group in groups" :key="group.source" class="group">
        <div class="group-head">
          <h2>
            <i class="dot" :style="{ background: SOURCE_META[group.source].color }" />
            {{ group.label }}
          </h2>
          <span class="muted small">
            {{ group.items.length ? `${group.items.length} 个歌单` : '' }}
          </span>
        </div>

        <template v-if="group.items.length">
          <!-- 大封面 -->
          <div v-if="view === 'grid'" class="grid">
            <article
              v-for="item in group.items"
              :key="`${item.source}-${item.id}`"
              class="pl-card"
              @click="open(item)"
            >
              <div class="cover">
                <img
                  :src="coverOf(item.cover, item.id)"
                  alt=""
                  loading="lazy"
                  @error="onCoverError($event, item.id)"
                />
                <span class="badge mono">{{ item.trackCount }}</span>
              </div>
              <p class="name" :title="item.name">{{ item.name }}</p>
              <p class="sub">{{ item.creator || SOURCE_META[item.source].label }}</p>
            </article>
          </div>

          <!-- 小图标 -->
          <div v-else-if="view === 'compact'" class="grid compact">
            <article
              v-for="item in group.items"
              :key="`${item.source}-${item.id}`"
              class="pl-card"
              @click="open(item)"
            >
              <div class="cover">
                <img
                  :src="coverOf(item.cover, item.id)"
                  alt=""
                  loading="lazy"
                  @error="onCoverError($event, item.id)"
                />
                <span class="badge mono">{{ item.trackCount }}</span>
              </div>
              <p class="name" :title="item.name">{{ item.name }}</p>
            </article>
          </div>

          <!-- 列表 -->
          <div v-else class="pl-rows">
            <article
              v-for="item in group.items"
              :key="`${item.source}-${item.id}`"
              class="pl-row"
              @click="open(item)"
            >
              <div class="thumb">
                <img
                  :src="coverOf(item.cover, item.id)"
                  alt=""
                  loading="lazy"
                  @error="onCoverError($event, item.id)"
                />
              </div>
              <div class="row-main">
                <p class="name" :title="item.name">{{ item.name }}</p>
                <p class="sub">
                  {{ item.creator || SOURCE_META[item.source].label }} · {{ item.trackCount }} 首
                </p>
              </div>
              <Icon name="chevron" :size="15" class="row-arrow" />
            </article>
          </div>
        </template>

        <p v-else class="hint">
          <template v-if="group.total && query.trim()">
            这一组没有匹配「{{ query.trim() }}」的歌单。
          </template>
          <template v-else-if="!group.loggedIn">
            还没有登录{{ group.label }}。
            <RouterLink to="/settings">去登录</RouterLink>
            后即可看到账号里的歌单。
          </template>
          <template v-else>
            没有读到歌单（该账号可能没有自建歌单，或登录状态已过期）。
          </template>
        </p>
      </section>

      <section v-if="!loading && !total && !error" class="empty">
        <div class="empty-art"><Icon name="queue" :size="26" /></div>
        <h3>还没有读到歌单</h3>
        <p class="muted">
          歌单来自你在各平台的账号。先在设置里登录网易云或 QQ 音乐，
          这里就会列出「我喜欢的音乐」「默认收藏」等歌单。
        </p>
        <RouterLink to="/settings" class="btn btn-primary" style="margin-top: 16px">
          去登录
        </RouterLink>
      </section>

      <section v-else-if="searching" class="empty">
        <div class="empty-art"><Icon name="search" :size="26" /></div>
        <h3>没有匹配的歌单</h3>
        <p class="muted">换个关键词试试，或清空搜索框。</p>
      </section>
    </template>
  </div>
</template>

<style scoped>
.view {
  padding: var(--view-pad);
  max-width: 1180px;
  margin: 0 auto;
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
  flex-wrap: wrap;
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
.head-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

/* 显示方式切换：图标比文字省地方，窄屏也放得下 */
.view-switch {
  display: inline-flex;
  gap: 2px;
  padding: 3px;
  border-radius: var(--r-sm);
  background: var(--surface-2);
}
.view-switch button {
  display: grid;
  place-items: center;
  width: 28px;
  height: 26px;
  border-radius: 7px;
  color: var(--text-3);
  transition: all var(--dur) var(--ease);
}
.view-switch button:hover {
  color: var(--text);
}
.view-switch button.on {
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-sm);
}

/* 搜索框：列表页和详情页共用 */
.field,
.list-bar,
.detail-bar {
  display: flex;
  align-items: center;
  gap: 10px;
}
.list-bar,
.detail-bar {
  margin: 0 0 16px;
  flex-wrap: wrap;
}
.field {
  flex: 1 1 260px;
  min-width: 200px;
  height: 34px;
  padding: 0 10px;
  border-radius: var(--r-sm);
  border: 1px solid var(--line);
  background: var(--surface);
  color: var(--text-3);
  transition: border-color var(--dur) var(--ease);
}
.field:focus-within {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--line));
}
.field input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: 0;
  background: transparent;
  color: var(--text);
  font-size: 12.5px;
  outline: none;
}
.field input::-webkit-search-cancel-button {
  display: none;
}
.field .clear {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  color: var(--text-3);
  background: var(--surface-2);
}
.chips {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.small {
  font-size: 11.5px;
}

.alert {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 16px;
  padding: 9px 12px;
  border-radius: var(--r-sm);
  background: color-mix(in srgb, var(--danger) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--danger) 32%, transparent);
  color: var(--danger);
  font-size: 12.5px;
}

.group {
  margin-bottom: 26px;
}
.group-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}
.group-head h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: 0 0 auto;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(158px, 1fr));
  gap: 14px;
}
.grid.compact {
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
  gap: 10px;
}
.grid.compact .cover {
  border-radius: 10px;
}
.grid.compact .name {
  margin-top: 6px;
  font-size: 11.5px;
  -webkit-line-clamp: 1;
  line-clamp: 1;
}

.pl-card {
  cursor: pointer;
  border-radius: var(--r-md);
  transition: transform var(--dur) var(--ease);
}
.pl-card:hover {
  transform: translateY(-2px);
}
.cover {
  position: relative;
  aspect-ratio: 1;
  border-radius: 12px;
  overflow: hidden;
  background: var(--surface-2);
  border: 1px solid var(--line);
}
.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.badge {
  position: absolute;
  right: 8px;
  bottom: 8px;
  min-width: 22px;
  padding: 2px 6px;
  border-radius: var(--r-full);
  background: rgba(10, 10, 16, 0.58);
  backdrop-filter: blur(6px);
  color: #fff;
  font-size: 10px;
  text-align: center;
}
.grid.compact .badge {
  right: 5px;
  bottom: 5px;
  min-width: 18px;
  padding: 1px 4px;
  font-size: 9px;
}
.name {
  margin: 8px 2px 0;
  font-size: 12.5px;
  font-weight: 500;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.sub {
  margin: 2px 2px 0;
  font-size: 11px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 列表式：一行一个歌单 */
.pl-rows {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.pl-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 7px 10px;
  border-radius: var(--r-sm);
  cursor: pointer;
  transition: background var(--dur) var(--ease);
}
.pl-row:hover {
  background: var(--hover);
}
.thumb {
  width: 44px;
  height: 44px;
  flex: 0 0 auto;
  border-radius: 9px;
  overflow: hidden;
  border: 1px solid var(--line);
  background: var(--surface-2);
}
.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.row-main {
  min-width: 0;
  flex: 1;
}
.row-main .name {
  margin: 0;
  -webkit-line-clamp: 1;
  line-clamp: 1;
}
.row-arrow {
  color: var(--text-3);
  transform: rotate(-90deg);
  flex: 0 0 auto;
}

.hint {
  margin: 0;
  padding: 14px 16px;
  border-radius: var(--r-sm);
  border: 1px dashed var(--line-strong);
  background: var(--surface);
  font-size: 12.5px;
  color: var(--text-3);
  line-height: 1.7;
}
.hint a {
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.more {
  display: flex;
  justify-content: center;
  padding: 18px 0 4px;
}
.end {
  font-size: 12px;
  margin: 0;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 64px 20px;
}
.empty-art {
  width: 62px;
  height: 62px;
  display: grid;
  place-items: center;
  border-radius: 20px;
  color: var(--accent);
  background: var(--active);
  margin-bottom: 16px;
}
.empty h3 {
  margin: 0 0 6px;
  font-size: 15px;
  font-weight: 500;
}
.empty p {
  margin: 0;
  font-size: 12.5px;
  max-width: 420px;
  line-height: 1.7;
}

.flip {
  transform: rotate(180deg);
}
.spin {
  animation: spin 1s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 860px) {
  .view {
    padding: calc(env(safe-area-inset-top, 0px) + 18px) 14px
      calc(env(safe-area-inset-bottom, 0px) + 26px);
  }
  .grid {
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 10px;
  }
  .grid.compact {
    grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
    gap: 8px;
  }
  .head-actions {
    width: 100%;
  }
  .detail-bar,
  .list-bar {
    gap: 8px;
  }
  .field {
    flex: 1 1 100%;
  }
}
</style>
