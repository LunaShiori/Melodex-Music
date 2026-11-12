<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'

import Icon from '@/components/Icon.vue'
import TrackRow from '@/components/TrackRow.vue'
import DownloadPicker from '@/components/DownloadPicker.vue'
import { coverOf } from '@/lib/artwork'
import { clearKeywords, clearRecent, keywords, pushKeyword, recent } from '@/lib/history'
import { sourceName, sourceShort, useI18n, artistName } from '@/lib/i18n'
import { ALL_SOURCES, SOURCE_META } from '@/lib/types'
import type { Track, Variant } from '@/lib/types'
import { SORT_MODES, useSearchStore } from '@/stores/search'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'

const route = useRoute()
const { t } = useI18n()
const search = useSearchStore()
const player = usePlayerStore()
const tasks = useTasksStore()

const inputEl = ref<HTMLInputElement | null>(null)
const hot = ['晴天', '周杰伦', '起风了', 'Lemon', '漠河舞厅', '夜空中最亮的星']

/** 多选下载。批量操作必须由用户明确勾选，不能一键把整页结果全塞进队列。 */
const selectMode = ref(false)
const picked = reactive(new Set<string>())

/** 渲染顺序 = 后端顺序经过「排序方式」处理后的结果 */
const list = computed(() => search.visible)

const summary = computed(() => {
  if (!search.searched) return ''
  return t('search.summary', { count: search.tracks.length, ms: search.elapsed })
})

const pickedTracks = computed(() => list.value.filter((item) => picked.has(item.fingerprint)))
const allPicked = computed(() => list.value.length > 0 && picked.size >= list.value.length)

/* 最近搜索下拉的状态。用 mousedown.prevent 而不是 click，避免 blur 先把面板收掉 */
const historyOpen = ref(false)

function openHistory() {
  historyOpen.value = keywords.value.length > 0
}

function closeHistory() {
  historyOpen.value = false
}

function submit() {
  const word = search.keyword.trim()
  if (word) pushKeywordSafe(word)
  void search.run(1, false)
}

/** 记历史是顺手的事，写不进去也不该拦住搜索 */
function pushKeywordSafe(word: string) {
  try {
    pushKeyword(word)
  } catch {
    /* localStorage 不可用 */
  }
}

function pick(word: string) {
  search.keyword = word
  submit()
}

function playTrack(track: Track) {
  const index = list.value.findIndex((item) => item.fingerprint === track.fingerprint)
  void player.playList(list.value, index < 0 ? 0 : index)
}

/** 从「{{ t('search.recentPlays') }}」起播：整个最近列表就是队列，位置对应该曲 */
function playRecent(track: Track) {
  const index = recent.value.findIndex((item) => item.fingerprint === track.fingerprint)
  void player.playList(recent.value, index < 0 ? 0 : index)
}

/** 单曲下载时弹出平台/音质选择框 */
const pickerTrack = ref<Track | null>(null)
const pickerSource = ref<string | null>(null)

function download(track: Track, variant?: Variant) {
  // 只有一个音源也弹窗：用户要先看到这档能不能下、能不能选，选完再入队
  // 点了某个音源芯片：预选该平台；否则默认按最高音质挑平台
  pickerSource.value = variant?.source ?? null
  pickerTrack.value = track
}

function onPickerConfirm(payload: { variant: Variant; quality: string; label: string }) {
  if (!pickerTrack.value) return
  void tasks.enqueuePicked(
    pickerTrack.value,
    payload.variant,
    payload.quality as import('@/lib/types').QualityLevel,
    payload.label,
  )
  pickerTrack.value = null
}

function downloadAll() {
  if (list.value.length) void tasks.enqueue(list.value)
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
  for (const track of list.value) picked.add(track.fingerprint)
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
  // 桌面端进来就能直接打字；手机端不自动聚焦——一进页面就弹输入法，
  // 会把底部导航和播放条一起顶掉，还得手动收回去，很难受。
  if (window.matchMedia('(min-width: 861px)').matches) inputEl.value?.focus()
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
        <h1>{{ t('search.title') }}</h1>
        <p class="muted">{{ t('search.subtitle') }}</p>
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
        :placeholder="t('search.placeholder')"
        autocomplete="off"
        spellcheck="false"
        @keydown.enter="submit"
        @focus="openHistory"
        @blur="closeHistory"
      />
      <!-- 最近搜索跟着搜索框走：聚焦就展开，点条目直接搜 -->
      <Transition name="drop">
        <div v-if="historyOpen && keywords.length" class="history-drop">
          <div class="drop-head">
            <span>{{ t('search.recentKeywords') }}</span>
            <button class="link-btn" type="button" @mousedown.prevent="clearKeywords()">{{ t('common.clear') }}</button>
          </div>
          <button
            v-for="word in keywords"
            :key="word"
            class="drop-item"
            type="button"
            @mousedown.prevent="pick(word)"
          >
            <Icon name="history" :size="14" />
            <span class="drop-word">{{ word }}</span>
          </button>
        </div>
      </Transition>
      <button v-if="search.keyword" class="clear" type="button" @click="search.reset()">
        <Icon name="close" :size="15" />
      </button>
      <kbd class="hint mono">Ctrl K</kbd>
      <button class="submit" type="button" :disabled="!search.keyword.trim() || search.loading" @click="submit">
        {{ search.loading ? t('common.searching') : t('common.search') }}
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
          {{ sourceName(source) }}
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
          {{ sourceShort(report.source) }}
          {{ report.ok ? report.count : t('search.timeout') }}
        </span>
      </div>

      <button
        v-if="search.tracks.length && !selectMode"
        class="btn btn-sm pick-btn"
        type="button"
        @click="enterSelect"
      >
        <Icon name="check" :size="14" />
        {{ t('search.multiSelect') }}
      </button>

      <button
        v-if="search.tracks.length && !selectMode"
        class="btn btn-primary btn-sm all"
        type="button"
        @click="downloadAll"
      >
        <Icon name="download" :size="14" />
        {{ t('search.downloadAll') }}
      </button>
    </div>

    <!-- 排序工具条 -->
    <div v-if="search.tracks.length" class="toolbar">
      <div class="segmented sort">
        <button
          v-for="mode in SORT_MODES"
          :key="mode.value"
          type="button"
          :class="{ on: search.sortMode === mode.value }"
          :title="t(mode.hint)"
          @click="search.setSort(mode.value)"
        >
          {{ t(mode.label) }}
        </button>
      </div>

      <span class="muted small sort-hint">
        {{ t(SORT_MODES.find((m) => m.value === search.sortMode)?.hint ?? 'search.sort.defaultHint') }}
      </span>
    </div>

    <div v-if="search.error" class="banner error">
      <Icon name="alert" :size="16" />
      <span>{{ search.error }}</span>
      <button class="btn btn-sm" type="button" @click="submit">{{ t('search.retry') }}</button>
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
        <span>{{ t('search.colSong') }}</span>
        <span>{{ t('search.colSources') }}</span>
        <span class="ta-r">{{ t('search.colQuality') }}</span>
        <span class="ta-r">{{ t('search.colDuration') }}</span>
        <span />
      </div>
      <TrackRow
        v-for="(track, index) in list"
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
          {{ search.loadingMore ? t('common.loading') : t('search.loadMore') }}
        </button>
        <p v-else class="muted end">{{ t('search.end') }}</p>
      </div>
    </section>

    <section v-else-if="search.searched" class="empty">
      <div class="empty-art"><Icon name="search" :size="26" /></div>
      <h3>{{ t('search.noResult', { keyword: search.activeKeyword }) }}</h3>
      <p class="muted">{{ t('search.emptyHint') }}</p>
    </section>

    <section v-else class="empty">
      <div class="empty-art"><Icon name="music" :size="26" /></div>
      <h3>{{ t('search.emptyTitle') }}</h3>
      <p class="muted">{{ t('search.emptyDesc') }}</p>

      <!-- {{ t('search.recentPlays') }}：点一条直接起播，曲库里有过的歌不用再搜 -->
      <div v-if="recent.length" class="history-block">
        <div class="history-head">
          <span>{{ t('search.recentPlays') }}</span>
          <button class="link-btn" type="button" @click="clearRecent()">{{ t('searchview.a1') }}</button>
        </div>
        <ul class="recent-list">
          <li v-for="track in recent.slice(0, 5)" :key="track.fingerprint">
            <button class="recent-item" type="button" @click="playRecent(track)">
              <img class="recent-cover" :src="coverOf(track.cover, track.fingerprint)" alt="" loading="lazy" />
              <span class="recent-text">
                <span class="recent-title">{{ track.title }}</span>
                <span class="recent-meta">{{ artistName(track.artists) }}</span>
              </span>
              <Icon name="play" :size="14" />
            </button>
          </li>
        </ul>
      </div>

      <div class="hot-label">{{ recent.length ? t('search.tryInstead') : t('search.hot') }}</div>
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
          {{ t('search.selectAll') }}
        </button>
        <span class="bulk-count mono">{{ t('search.picked', { n: picked.size }) }}</span>
        <button class="btn btn-sm" type="button" @click="exitSelect">{{ t('common.cancel') }}</button>
        <button
          class="btn btn-primary btn-sm"
          type="button"
          :disabled="!picked.size"
          @click="downloadPicked"
        >
          <Icon name="download" :size="14" />{{ t('searchview.a2') }}</button>
      </div>
    </Transition>

    <DownloadPicker
      v-if="pickerTrack"
      :track="pickerTrack"
      :initial-source="pickerSource"
      @confirm="onPickerConfirm"
      @close="pickerTrack = null"
    />
  </div>
</template>

<style scoped>
.view {
  padding: var(--view-pad);
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
  position: relative;
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

/* 排序工具条 */
.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 0 10px;
  flex-wrap: wrap;
}
.sort button {
  min-width: 42px;
}
.sort-hint {
  font-size: 11.5px;
  color: var(--text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
/* 最近搜索下拉：贴在搜索框下面，和搜索框同宽 */
.history-drop {
  position: absolute;
  z-index: 30;
  top: calc(100% + 8px);
  left: 0;
  right: 0;
  padding: 6px;
  border-radius: var(--r-lg);
  border: 1px solid var(--line);
  background: linear-gradient(var(--surface), var(--surface)), var(--bg);
  box-shadow: var(--shadow-lg);
  max-height: 320px;
  overflow-y: auto;
}
.drop-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px 6px;
  font-size: 11.5px;
  color: var(--text-3);
}
.drop-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border: 0;
  border-radius: var(--r-sm);
  background: transparent;
  color: var(--text-2);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}
.drop-item:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
  color: var(--text);
}
.drop-word {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.drop-enter-active,
.drop-leave-active {
  transition: opacity 120ms var(--ease), transform 120ms var(--ease);
}
.drop-enter-from,
.drop-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.hot {
  display: flex;
  gap: 8px;
  margin-top: 18px;
  flex-wrap: wrap;
  justify-content: center;
}
.hot-label {
  margin-top: 26px;
  font-size: 12px;
  color: var(--text-2);
}
.hot-label + .hot {
  margin-top: 10px;
}

/* 空态里的最近播放 / 最近搜索 */
.history-block {
  width: 100%;
  max-width: 440px;
  margin-top: 24px;
}
.history-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  font-size: 12px;
  color: var(--text-2);
}
.history-head .link-btn {
  border: 0;
  background: none;
  padding: 2px 4px;
  font-size: 12px;
  color: var(--text-2);
  cursor: pointer;
  border-radius: 6px;
}
.history-head .link-btn:hover {
  color: var(--accent);
}
.recent-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.recent-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 8px;
  border: 0;
  border-radius: 10px;
  background: none;
  color: inherit;
  text-align: left;
  cursor: pointer;
}
.recent-item:hover {
  background: var(--surface-2);
}
.recent-cover {
  width: 34px;
  height: 34px;
  border-radius: 8px;
  object-fit: cover;
  flex: none;
}
.recent-text {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}
.recent-title {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.recent-meta {
  font-size: 11px;
  color: var(--text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
    padding: calc(env(safe-area-inset-top, 0px) + 18px) 14px
      calc(env(safe-area-inset-bottom, 0px) + 28px);
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
  /* 手机上把排序说明收起来：一行放不下，说明文字会自己挤到第二行 */
  .sort-hint {
    display: none;
  }
  .toolbar {
    gap: 8px;
    margin-bottom: 8px;
  }
  .sort button {
    min-width: 36px;
    padding: 0 8px;
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
