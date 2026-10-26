<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'

import Icon from '@/components/Icon.vue'
import TrackRow from '@/components/TrackRow.vue'
import { ALL_SOURCES, SOURCE_META } from '@/lib/types'
import type { Track, Variant } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'
import { useSearchStore } from '@/stores/search'
import { useTasksStore } from '@/stores/tasks'

const route = useRoute()
const search = useSearchStore()
const player = usePlayerStore()
const tasks = useTasksStore()

const inputEl = ref<HTMLInputElement | null>(null)
const hot = ['晴天', '周杰伦', '起风了', 'Lemon', '漠河舞厅', '夜空中最亮的星']

/** 多选下载。批量操作必须由用户明确勾选，不能一键把整页结果全塞进队列。 */
const selectMode = ref(false)
const picked = reactive(new Set<string>())

const summary = computed(() => {
  if (!search.searched) return ''
  return `${search.tracks.length} 首 · ${search.elapsed}ms`
})

const pickedTracks = computed(() => search.tracks.filter((item) => picked.has(item.fingerprint)))
const allPicked = computed(
  () => search.tracks.length > 0 && picked.size >= search.tracks.length,
)

function submit() {
  void search.run(1, false)
}

function pick(word: string) {
  search.keyword = word
  submit()
}

function playTrack(track: Track) {
  const index = search.tracks.findIndex((item) => item.fingerprint === track.fingerprint)
  void player.playList(search.tracks, index < 0 ? 0 : index)
}

function download(track: Track, _variant?: Variant) {
  void tasks.enqueue([track])
}

function downloadAll() {
  if (search.tracks.length) void tasks.enqueue(search.tracks)
}

function enterSelect() {
  selectMode.value = true
  picked.clear()
}

function exitSelect() {
  selectMode.value = false
  picked.clear()
}

function togglePick(track: Track) {
  if (picked.has(track.fingerprint)) picked.delete(track.fingerprint)
  else picked.add(track.fingerprint)
}

function toggleAll() {
  if (allPicked.value) {
    picked.clear()
    return
  }
  for (const track of search.tracks) picked.add(track.fingerprint)
}

function downloadPicked() {
  const list = pickedTracks.value
  if (!list.length) return
  void tasks.enqueue(list)
  exitSelect()
}

function onKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault()
    inputEl.value?.focus()
  }
  if (event.key === 'Escape' && selectMode.value) exitSelect()
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  inputEl.value?.focus()
  // 支持 #/search?q=关键词 直接带参搜索
  const initial = route.query.q
  if (typeof initial === 'string' && initial.trim()) {
    search.keyword = initial
    submit()
  }
})
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="view">
    <header class="head">
      <div class="head-text">
        <h1>聚合搜索</h1>
        <p class="muted">一次输入，同时检索网易云、QQ 音乐与酷我，同曲自动合并。</p>
      </div>
      <div class="head-stat mono" v-if="search.searched">
        <span>{{ summary }}</span>
      </div>
    </header>

    <div class="searchbar" :class="{ filled: !!search.keyword }">
      <Icon name="search" :size="18" />
      <input
        ref="inputEl"
        v-model="search.keyword"
        type="search"
        placeholder="搜索歌曲、歌手，或粘贴歌单链接"
        autocomplete="off"
        spellcheck="false"
        @keydown.enter="submit"
      />
      <button v-if="search.keyword" class="clear" type="button" @click="search.reset()">
        <Icon name="close" :size="15" />
      </button>
      <kbd class="hint mono">Ctrl K</kbd>
      <button class="submit" type="button" :disabled="!search.keyword.trim() || search.loading" @click="submit">
        {{ search.loading ? '搜索中' : '搜索' }}
      </button>
    </div>

    <div class="filters">
      <div class="chips">
        <button
          v-for="source in ALL_SOURCES"
          :key="source"
          class="chip chip-btn"
          :class="{ 'is-on': search.sources.includes(source) }"
          type="button"
          @click="search.toggleSource(source)"
        >
          <i class="dot" :style="{ background: SOURCE_META[source].color }" />
          {{ SOURCE_META[source].label }}
        </button>
      </div>

      <div class="reports" v-if="search.reports.length">
        <span
          v-for="report in search.reports"
          :key="report.source"
          class="report mono"
          :class="{ bad: !report.ok }"
          :title="report.error ?? ''"
        >
          <i :style="{ background: report.ok ? SOURCE_META[report.source].color : 'var(--danger)' }" />
          {{ SOURCE_META[report.source].short }}
          {{ report.ok ? report.count : '超时' }}
        </span>
      </div>

      <button
        v-if="search.tracks.length && !selectMode"
        class="btn btn-sm pick-btn"
        type="button"
        @click="enterSelect"
      >
        <Icon name="check" :size="14" />
        多选
      </button>

      <button
        v-if="search.tracks.length && !selectMode"
        class="btn btn-primary btn-sm all"
        type="button"
        @click="downloadAll"
      >
        <Icon name="download" :size="14" />
        全部下载
      </button>
    </div>

    <div v-if="search.error" class="banner error">
      <Icon name="alert" :size="16" />
      <span>{{ search.error }}</span>
      <button class="btn btn-sm" type="button" @click="submit">重试</button>
    </div>

    <section v-if="search.loading" class="list">
      <div v-for="i in 6" :key="i" class="skeleton">
        <span class="sk-cover" />
        <span class="sk-lines">
          <span class="sk-line w60" />
          <span class="sk-line w35" />
        </span>
      </div>
    </section>

    <section v-else-if="search.tracks.length" class="list">
      <div class="list-head">
        <span>#</span>
        <span />
        <span>歌曲</span>
        <span>可用音源</span>
        <span class="ta-r">音质</span>
        <span class="ta-r">时长</span>
        <span />
      </div>
      <TrackRow
        v-for="(track, index) in search.tracks"
        :key="track.fingerprint"
        :track="track"
        :index="index"
        :active="player.isCurrent(track)"
        :playing="player.playing"
        :select-mode="selectMode"
        :selected="picked.has(track.fingerprint)"
        @play="playTrack"
        @download="download"
        @toggle="togglePick"
      />
      <div class="more">
        <button
          v-if="search.hasMore"
          class="btn"
          type="button"
          :disabled="search.loadingMore"
          @click="search.run(search.page + 1, true)"
        >
          {{ search.loadingMore ? '加载中…' : '加载更多' }}
        </button>
        <p v-else class="muted end">已到末尾</p>
      </div>
    </section>

    <section v-else-if="search.searched" class="empty">
      <div class="empty-art"><Icon name="search" :size="26" /></div>
      <h3>没有找到「{{ search.activeKeyword }}」</h3>
      <p class="muted">换个关键词，或检查上方的音源是否全部关闭。</p>
    </section>

    <section v-else class="empty">
      <div class="empty-art"><Icon name="music" :size="26" /></div>
      <h3>搜索你想听的歌</h3>
      <p class="muted">结果会按音源合并，同一首歌只出现一次，右侧标签表示可从哪些平台取到。</p>
      <div class="hot">
        <button v-for="word in hot" :key="word" class="chip chip-btn" type="button" @click="pick(word)">
          {{ word }}
        </button>
      </div>
    </section>

    <!-- 批量操作条：只对勾选的歌曲生效 -->
    <Transition name="bulk">
      <div v-if="selectMode" class="bulk">
        <button class="bulk-all" type="button" @click="toggleAll">
          <span class="bulk-box" :class="{ on: allPicked }">
            <Icon v-if="allPicked" name="check" :size="12" />
          </span>
          全选
        </button>
        <span class="bulk-count mono">已选 {{ picked.size }} 首</span>
        <button class="btn btn-sm" type="button" @click="exitSelect">取消</button>
        <button
          class="btn btn-primary btn-sm"
          type="button"
          :disabled="!picked.size"
          @click="downloadPicked"
        >
          <Icon name="download" :size="14" />
          下载
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.view {
  padding: 26px 26px 40px;
  max-width: 1180px;
  margin: 0 auto;
  position: relative;
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}
.head-text {
  min-width: 0;
}
.head h1 {
  margin: 0;
  font-size: 22px;
  font-weight: 600;
  letter-spacing: -0.01em;
}
.head p {
  margin: 5px 0 0;
  font-size: 12.5px;
}
.head-stat {
  flex: 0 0 auto;
  white-space: nowrap;
  font-size: 11.5px;
  color: var(--text-3);
  padding: 4px 10px;
  border-radius: var(--r-full);
  border: 1px solid var(--line);
}

.searchbar {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 52px;
  padding: 0 8px 0 16px;
  border-radius: var(--r-lg);
  background: var(--surface);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-sm);
  transition: border-color var(--dur) var(--ease), box-shadow var(--dur) var(--ease);
  color: var(--text-3);
}
.searchbar:focus-within {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 12%, transparent);
}
.searchbar input {
  flex: 1;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: 14.5px;
  color: var(--text);
  min-width: 0;
}
.searchbar input::placeholder {
  color: var(--text-3);
}
.searchbar input::-webkit-search-cancel-button {
  display: none;
}
.hint {
  font-size: 10px;
  color: var(--text-3);
  border: 1px solid var(--line);
  border-radius: var(--r-xs);
  padding: 2px 5px;
}
.clear {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  color: var(--text-3);
}
.clear:hover {
  background: var(--hover);
  color: var(--text);
}
.submit {
  height: 38px;
  padding: 0 18px;
  border-radius: 12px;
  background: var(--accent);
  color: #fff;
  font-size: 13px;
  font-weight: 500;
  transition: filter var(--dur) var(--ease);
}
.submit:hover {
  filter: brightness(1.08);
}
.submit:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.filters {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 14px 0 6px;
  flex-wrap: wrap;
}
.chips {
  display: flex;
  gap: 6px;
}
.reports {
  display: flex;
  gap: 10px;
  margin-left: 2px;
}
.report {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 10.5px;
  color: var(--text-3);
}
.report i {
  width: 5px;
  height: 5px;
  border-radius: 50%;
}
.report.bad {
  color: var(--danger);
}
.all {
  margin-left: auto;
}
.pick-btn {
  margin-left: auto;
}
.pick-btn + .all {
  margin-left: 0;
}

/* 批量操作条：浮在播放条之上，不挤压列表 */
.bulk {
  position: fixed;
  left: 50%;
  transform: translateX(-50%);
  bottom: calc(var(--player) + 16px);
  z-index: 25;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px 9px 14px;
  border-radius: var(--r-full);
  border: 1px solid var(--line-strong);
  background: var(--glass);
  backdrop-filter: blur(22px) saturate(1.6);
  box-shadow: var(--shadow-lg);
  white-space: nowrap;
}
.bulk-all {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  color: var(--text-2);
}
.bulk-box {
  display: grid;
  place-items: center;
  width: 17px;
  height: 17px;
  border-radius: 5px;
  border: 1.5px solid var(--line-strong);
  color: #fff;
}
.bulk-box.on {
  background: var(--accent);
  border-color: var(--accent);
}
.bulk-count {
  font-size: 11.5px;
  color: var(--text-2);
  min-width: 62px;
}
.bulk-enter-active,
.bulk-leave-active {
  transition: opacity 180ms var(--ease), transform 220ms var(--ease);
}
.bulk-enter-from,
.bulk-leave-to {
  opacity: 0;
  transform: translate(-50%, 10px);
}

.banner {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 12px 0;
  padding: 10px 14px;
  border-radius: var(--r-md);
  font-size: 12.5px;
}
.banner.error {
  background: color-mix(in srgb, var(--danger) 10%, transparent);
  color: var(--danger);
}
.banner .btn {
  margin-left: auto;
}

.list {
  margin-top: 8px;
  display: flex;
  flex-direction: column;
}

.list-head {
  display: grid;
  grid-template-columns: 24px 44px minmax(0, 1fr) 172px 76px 48px 72px;
  gap: 12px;
  align-items: center;
  padding: 0 12px 8px;
  font-size: 10.5px;
  letter-spacing: 0.06em;
  color: var(--text-3);
  border-bottom: 1px solid var(--line);
  margin-bottom: 4px;
}
.list-head .ta-r {
  text-align: right;
}

.more {
  display: grid;
  place-items: center;
  padding: 18px 0 0;
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
  padding: 68px 20px 40px;
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
  max-width: 460px;
  line-height: 1.7;
}
.hot {
  display: flex;
  gap: 8px;
  margin-top: 18px;
  flex-wrap: wrap;
  justify-content: center;
}

.skeleton {
  display: flex;
  align-items: center;
  gap: 14px;
  height: 60px;
  padding: 0 12px;
}
.sk-cover {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  background: var(--surface-2);
  animation: pulse 1.2s ease-in-out infinite;
}
.sk-lines {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 7px;
}
.sk-line {
  height: 10px;
  border-radius: 5px;
  background: var(--surface-2);
  animation: pulse 1.2s ease-in-out infinite;
}
.w60 {
  width: 58%;
}
.w35 {
  width: 32%;
}
@keyframes pulse {
  0%,
  100% {
    opacity: 0.55;
  }
  50% {
    opacity: 1;
  }
}

@media (max-width: 860px) {
  .view {
    padding: 18px 14px 28px;
  }
  .head {
    align-items: flex-start;
    gap: 10px;
    margin-bottom: 14px;
  }
  .head h1 {
    font-size: 19px;
  }
  .head p {
    font-size: 12px;
  }
  .head-stat {
    margin-top: 2px;
  }
  .searchbar {
    height: 46px;
    padding-left: 12px;
    /* 滚动时搜索框吸顶，配合毛玻璃让下面的列表透出来 */
    position: sticky;
    top: 0;
    z-index: 6;
    background: color-mix(in srgb, var(--surface) 66%, transparent);
    backdrop-filter: blur(22px) saturate(1.7);
    box-shadow: 0 8px 24px -18px rgba(0, 0, 0, 0.6);
  }
  .hint {
    display: none;
  }
  .list-head {
    display: none;
  }
  .submit {
    height: 34px;
    padding: 0 14px;
  }
  /* 手机端要给播放条 + 底部标签栏让出位置 */
  .bulk {
    bottom: calc(env(safe-area-inset-bottom, 0px) + 128px);
    gap: 8px;
    padding: 8px 10px 8px 12px;
  }
  .pick-btn {
    margin-left: auto;
  }
}
</style>
