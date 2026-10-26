<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import Icon from '@/components/Icon.vue'
import { coverOf } from '@/lib/artwork'
import { formatBitrate, formatBytes, formatDuration } from '@/lib/format'
import { transport } from '@/lib/transport'
import { SOURCE_META } from '@/lib/types'
import type { DownloadTask, Track } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'

type ViewMode = 'grid' | 'list'
type CoverSize = 'sm' | 'md' | 'lg'
type SortKey = 'recent' | 'title' | 'artist' | 'size' | 'duration'

const STORE_KEY = 'melodex.library.view'

const tasks = useTasksStore()
const player = usePlayerStore()
const keyword = ref('')

function readPrefs(): { mode: ViewMode; size: CoverSize; sort: SortKey } {
  const fallback = { mode: 'grid' as ViewMode, size: 'md' as CoverSize, sort: 'recent' as SortKey }
  if (typeof localStorage === 'undefined') return fallback
  try {
    const raw = localStorage.getItem(STORE_KEY)
    if (!raw) return fallback
    const parsed = JSON.parse(raw) as Partial<{ mode: ViewMode; size: CoverSize; sort: SortKey }>
    return {
      mode: parsed.mode === 'list' ? 'list' : 'grid',
      size: parsed.size === 'sm' || parsed.size === 'lg' ? parsed.size : 'md',
      sort: (['recent', 'title', 'artist', 'size', 'duration'] as SortKey[]).includes(
        parsed.sort as SortKey,
      )
        ? (parsed.sort as SortKey)
        : 'recent',
    }
  } catch {
    return fallback
  }
}

const prefs = readPrefs()
const mode = ref<ViewMode>(prefs.mode)
const size = ref<CoverSize>(prefs.size)
const sort = ref<SortKey>(prefs.sort)

watch([mode, size, sort], () => {
  if (typeof localStorage === 'undefined') return
  // 展示偏好只存在本地，不进设置文件（否则每次切换都要写盘 + 走一次 IPC）
  localStorage.setItem(
    STORE_KEY,
    JSON.stringify({ mode: mode.value, size: size.value, sort: sort.value }),
  )
})

const items = computed(() => {
  const list = [...tasks.completed].sort((a, b) => (b.finishedAt ?? 0) - (a.finishedAt ?? 0))
  const key = keyword.value.trim().toLowerCase()
  const filtered = !key
    ? list
    : list.filter(
        (task) =>
          task.title.toLowerCase().includes(key) ||
          task.artists.some((artist) => artist.toLowerCase().includes(key)) ||
          task.album.toLowerCase().includes(key),
      )

  const sorted = [...filtered]
  switch (sort.value) {
    case 'title':
      sorted.sort((a, b) => a.title.localeCompare(b.title, 'zh-Hans-CN'))
      break
    case 'artist':
      sorted.sort((a, b) =>
        (a.artists[0] ?? '').localeCompare(b.artists[0] ?? '', 'zh-Hans-CN'),
      )
      break
    case 'size':
      sorted.sort((a, b) => (b.total || 0) - (a.total || 0))
      break
    case 'duration':
      sorted.sort((a, b) => (b.durationMs || 0) - (a.durationMs || 0))
      break
    default:
      break
  }
  return sorted
})

const totalSize = computed(() => items.value.reduce((sum, task) => sum + (task.total || 0), 0))

/** 封面网格的列宽：小/中/大三档，用 CSS 变量驱动 auto-fill */
const gridMin = computed(() =>
  size.value === 'sm' ? '132px' : size.value === 'lg' ? '248px' : '186px',
)

function toTrack(task: DownloadTask): Track {
  return {
    fingerprint: task.id,
    title: task.title,
    artists: task.artists,
    album: task.album,
    durationMs: task.durationMs,
    cover: task.cover,
    bitrate: task.bitrate,
    format: task.format,
    fee: 'unknown',
    variants: [
      {
        source: task.source,
        id: task.sourceId,
        title: task.title,
        artists: task.artists,
        album: task.album,
        durationMs: task.durationMs,
        cover: task.cover,
        bitrate: task.bitrate,
        format: task.format,
        size: task.total,
        fee: 'unknown',
      },
    ],
  }
}

function play(task: DownloadTask) {
  const track = toTrack(task)
  const src = task.path ? transport.localUrl(task.path) : ''
  void player.playLocal(track, src)
}

const sortOptions: Array<{ value: SortKey; label: string }> = [
  { value: 'recent', label: '最近添加' },
  { value: 'title', label: '标题' },
  { value: 'artist', label: '歌手' },
  { value: 'size', label: '文件大小' },
  { value: 'duration', label: '时长' },
]
</script>

<template>
  <div class="view">
    <header class="head">
      <div>
        <h1>本地曲库</h1>
        <p class="muted">
          已下载 {{ items.length }} 首 · 共 {{ formatBytes(totalSize) }} · 文件按命名模板落盘
        </p>
      </div>
      <div class="search">
        <Icon name="search" :size="15" />
        <input v-model="keyword" type="search" placeholder="在曲库中查找" />
      </div>
    </header>

    <div class="toolbar">
      <div class="segmented">
        <button
          v-for="option in sortOptions"
          :key="option.value"
          type="button"
          :class="{ on: sort === option.value }"
          @click="sort = option.value"
        >
          {{ option.label }}
        </button>
      </div>

      <div class="right">
        <div v-if="mode === 'grid'" class="segmented size">
          <button
            v-for="option in [
              { value: 'sm' as const, label: '小' },
              { value: 'md' as const, label: '中' },
              { value: 'lg' as const, label: '大' },
            ]"
            :key="option.value"
            type="button"
            :class="{ on: size === option.value }"
            @click="size = option.value"
          >
            {{ option.label }}
          </button>
        </div>
        <div class="segmented view-toggle">
          <button
            type="button"
            :class="{ on: mode === 'grid' }"
            title="封面网格"
            @click="mode = 'grid'"
          >
            <Icon name="disc" :size="14" />
          </button>
          <button
            type="button"
            :class="{ on: mode === 'list' }"
            title="列表"
            @click="mode = 'list'"
          >
            <Icon name="queue" :size="14" />
          </button>
        </div>
      </div>
    </div>

    <section v-if="items.length && mode === 'grid'" class="grid" :style="{ '--min': gridMin }">
      <article v-for="task in items" :key="task.id" class="card">
        <div class="cover" @click="play(task)">
          <img :src="coverOf(task.cover, task.id)" alt="" loading="lazy" />
          <button class="play" type="button" title="播放">
            <Icon name="play" :size="16" :filled="true" />
          </button>
        </div>
        <div class="info">
          <p class="title" :title="task.title">{{ task.title }}</p>
          <p class="sub">{{ task.artists.join(' / ') || '未知歌手' }}</p>
          <p class="meta mono">
            <span :style="{ color: SOURCE_META[task.source].color }">
              {{ SOURCE_META[task.source].short }}
            </span>
            <span>{{ formatBitrate(task.bitrate) }}</span>
            <span>{{ formatDuration(task.durationMs) }}</span>
          </p>
        </div>
        <div class="ops">
          <button
            v-if="task.path"
            class="icon-btn"
            type="button"
            title="打开文件位置"
            @click="tasks.reveal(task.path!)"
          >
            <Icon name="folder" :size="15" />
          </button>
          <button class="icon-btn" type="button" title="移出列表" @click="tasks.remove(task.id)">
            <Icon name="close" :size="15" />
          </button>
        </div>
      </article>
    </section>

    <section v-else-if="items.length" class="rows">
      <div class="row-head">
        <span>标题</span>
        <span>音源</span>
        <span>音质</span>
        <span>大小</span>
        <span>时长</span>
        <span />
      </div>
      <article
        v-for="task in items"
        :key="task.id"
        class="row"
        @dblclick="play(task)"
      >
        <div class="row-main">
          <img class="thumb" :src="coverOf(task.cover, task.id)" alt="" loading="lazy" />
          <div class="row-text">
            <p class="title" :title="task.title">{{ task.title }}</p>
            <p class="sub">{{ task.artists.join(' / ') || '未知歌手' }}</p>
          </div>
          <button class="icon-btn row-play" type="button" title="播放" @click="play(task)">
            <Icon name="play" :size="14" :filled="true" />
          </button>
        </div>
        <span class="cell mono" :style="{ color: SOURCE_META[task.source].color }">
          {{ SOURCE_META[task.source].short }}
        </span>
        <span class="cell mono">{{ formatBitrate(task.bitrate) }}</span>
        <span class="cell mono">{{ formatBytes(task.total || 0) }}</span>
        <span class="cell mono">{{ formatDuration(task.durationMs) }}</span>
        <div class="cell ops">
          <button
            v-if="task.path"
            class="icon-btn"
            type="button"
            title="打开文件位置"
            @click="tasks.reveal(task.path!)"
          >
            <Icon name="folder" :size="15" />
          </button>
          <button class="icon-btn" type="button" title="移出列表" @click="tasks.remove(task.id)">
            <Icon name="close" :size="15" />
          </button>
        </div>
      </article>
    </section>

    <section v-else class="empty">
      <div class="empty-art"><Icon name="library" :size="26" /></div>
      <h3>{{ keyword ? '没有匹配的歌曲' : '曲库还是空的' }}</h3>
      <p class="muted">
        下载完成的歌曲会自动出现在这里。桌面端可以直接播放本地文件，移动端会打开系统播放器。
      </p>
      <RouterLink to="/search" class="btn btn-primary" style="margin-top: 16px">去搜索</RouterLink>
    </section>
  </div>
</template>

<style scoped>
.view {
  padding: 26px 26px 40px;
  max-width: 1180px;
  margin: 0 auto;
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
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

.search {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  padding: 0 12px;
  border-radius: var(--r-sm);
  border: 1px solid var(--line);
  background: var(--surface);
  color: var(--text-3);
  min-width: 220px;
}
.search input {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  font-size: 12.5px;
  color: var(--text);
  min-width: 0;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}
.toolbar .right {
  display: flex;
  align-items: center;
  gap: 8px;
}
.toolbar .segmented button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 30px;
}

/* ---------- 封面网格 ---------- */
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--min, 186px), 1fr));
  gap: 14px;
}

.card {
  border-radius: var(--r-md);
  border: 1px solid var(--line);
  background: var(--surface);
  padding: 10px;
  transition: transform var(--dur) var(--ease), box-shadow var(--dur) var(--ease);
}
.card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

.cover {
  position: relative;
  aspect-ratio: 1;
  border-radius: 10px;
  overflow: hidden;
  background: var(--surface-2);
  cursor: pointer;
}
.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.play {
  position: absolute;
  right: 8px;
  bottom: 8px;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  color: #fff;
  background: rgba(12, 12, 18, 0.55);
  backdrop-filter: blur(8px);
  opacity: 0;
  transform: translateY(4px);
  transition: all var(--dur) var(--ease);
}
.card:hover .play {
  opacity: 1;
  transform: translateY(0);
}

.info {
  padding: 9px 2px 2px;
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sub {
  margin: 2px 0 6px;
  font-size: 11.5px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.meta {
  display: flex;
  gap: 8px;
  font-size: 10px;
  color: var(--text-3);
}

.ops {
  display: flex;
  justify-content: flex-end;
  gap: 2px;
  margin-top: 4px;
}

/* ---------- 列表 ---------- */
.rows {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  background: var(--surface);
  overflow: hidden;
}
.row-head {
  display: grid;
  grid-template-columns: 1fr 62px 76px 82px 62px 76px;
  gap: 10px;
  padding: 9px 14px;
  border-bottom: 1px solid var(--line);
  font-size: 10.5px;
  letter-spacing: 0.06em;
  color: var(--text-3);
  text-transform: uppercase;
}
.row {
  display: grid;
  grid-template-columns: 1fr 62px 76px 82px 62px 76px;
  gap: 10px;
  align-items: center;
  padding: 8px 14px;
  border-bottom: 1px solid var(--line);
  transition: background var(--dur) var(--ease);
}
.row:last-child {
  border-bottom: none;
}
.row:hover {
  background: var(--hover);
}

.row-main {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.thumb {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  object-fit: cover;
  background: var(--surface-2);
  flex: 0 0 auto;
}
.row-text {
  min-width: 0;
  flex: 1;
}
.row-text .sub {
  margin: 1px 0 0;
}
.row-play {
  opacity: 0;
  transition: opacity var(--dur) var(--ease);
}
.row:hover .row-play {
  opacity: 1;
}

.cell {
  font-size: 11px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.row .ops {
  margin: 0;
  justify-content: flex-end;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 72px 20px;
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
  max-width: 400px;
}

@media (max-width: 860px) {
  .view {
    padding: 18px 14px 26px;
  }
  .grid {
    gap: 10px;
  }
  /* 手机上列表没空间摆六列，退化成「标题 + 操作」，其余信息折进副标题 */
  .row-head {
    display: none;
  }
  .row {
    grid-template-columns: 1fr auto;
    padding: 8px 10px;
  }
  .row .cell {
    display: none;
  }
  .row .ops {
    display: flex;
  }
  .row-play {
    opacity: 1;
  }
}
</style>
