<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'

import Icon from '@/components/Icon.vue'
import TrackRow from '@/components/TrackRow.vue'
import { coverOf } from '@/lib/artwork'
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

const groups = computed(() => {
  const wanted = filter.value === 'all' ? ACCOUNT_SOURCES : [filter.value]
  return wanted.map((source) => ({
    source,
    label: SOURCE_META[source].label,
    loggedIn: Boolean(accounts.accountOf(source)),
    items: playlists.value.filter((item) => item.source === source),
  }))
})

const total = computed(() => playlists.value.length)

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

const tracks = computed(() => (opened.value?.items ?? []).map(toTrack))

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
  if (!tracks.value.length) return
  void tasks.enqueue(tracks.value)
}

function downloadOne(track: Track) {
  void tasks.enqueue([track])
}

function playTrack(track: Track) {
  const index = tracks.value.findIndex((item) => item.fingerprint === track.fingerprint)
  void player.playList(tracks.value, index < 0 ? 0 : index)
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
          <button
            class="btn btn-primary"
            type="button"
            :disabled="!tracks.length"
            @click="downloadAll"
          >
            <Icon name="download" :size="14" />
            全部下载
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
          <button class="icon-btn" type="button" title="刷新" @click="load">
            <Icon name="refresh" :size="15" :class="{ spin: loading }" />
          </button>
        </template>
      </div>
    </header>

    <p v-if="error" class="alert">
      <Icon name="alert" :size="14" />
      {{ error }}
    </p>

    <!-- 歌单详情 -->
    <template v-if="opened">
      <section v-if="tracks.length" class="list">
        <TrackRow
          v-for="(track, index) in tracks"
          :key="track.fingerprint"
          :track="track"
          :index="index"
          :active="player.current?.fingerprint === track.fingerprint"
          :playing="player.playing"
          @play="playTrack"
          @download="downloadOne"
        />
      </section>
      <section v-else class="empty">
        <div class="empty-art"><Icon name="music" :size="26" /></div>
        <h3>这个歌单是空的</h3>
        <p class="muted">换一个歌单试试，或者去搜索页找歌。</p>
      </section>
    </template>

    <!-- 歌单列表 -->
    <template v-else>
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

        <div v-if="group.items.length" class="grid">
          <article
            v-for="item in group.items"
            :key="`${item.source}-${item.id}`"
            class="pl-card"
            @click="open(item)"
          >
            <div class="cover">
              <img :src="coverOf(item.cover, item.id)" alt="" loading="lazy" />
              <span class="badge mono">{{ item.trackCount }}</span>
            </div>
            <p class="name" :title="item.name">{{ item.name }}</p>
            <p class="sub">{{ item.creator || SOURCE_META[item.source].label }}</p>
          </article>
        </div>

        <p v-else class="hint">
          <template v-if="!group.loggedIn">
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
    </template>
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
.small {
  font-size: 11.5px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(158px, 1fr));
  gap: 14px;
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
    padding: 18px 14px 26px;
  }
  .grid {
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 10px;
  }
}
</style>
