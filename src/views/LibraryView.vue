<script setup lang="ts">
import { computed, ref } from 'vue'

import Icon from '@/components/Icon.vue'
import { coverOf } from '@/lib/artwork'
import { formatBitrate, formatBytes, formatDuration } from '@/lib/format'
import { transport } from '@/lib/transport'
import { SOURCE_META } from '@/lib/types'
import type { DownloadTask, Track } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'

const tasks = useTasksStore()
const player = usePlayerStore()
const keyword = ref('')

const items = computed(() => {
  const list = [...tasks.completed].sort((a, b) => (b.finishedAt ?? 0) - (a.finishedAt ?? 0))
  const key = keyword.value.trim().toLowerCase()
  if (!key) return list
  return list.filter(
    (task) =>
      task.title.toLowerCase().includes(key) ||
      task.artists.some((artist) => artist.toLowerCase().includes(key)) ||
      task.album.toLowerCase().includes(key),
  )
})

const totalSize = computed(() => items.value.reduce((sum, task) => sum + (task.total || 0), 0))

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

    <section v-if="items.length" class="grid">
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
  margin-bottom: 20px;
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

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(216px, 1fr));
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
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 10px;
  }
}
</style>
