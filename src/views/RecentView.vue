<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import Icon from '@/components/Icon.vue'
import TrackRow from '@/components/TrackRow.vue'
import { clearKeywords, clearRecent, keywords, loadHistory, recent } from '@/lib/history'
import { formatDuration } from '@/lib/format'
import type { Track } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'

const player = usePlayerStore()
const tasks = useTasksStore()
const router = useRouter()

loadHistory()

const tracks = computed(() => recent.value)
const totalMs = computed(() => tracks.value.reduce((sum, track) => sum + (track.durationMs || 0), 0))

/** 最近播放就是队列：从点中的那首往后放，顺序和顺序模式一致 */
function play(track: Track) {
  const at = tracks.value.findIndex((item) => item.fingerprint === track.fingerprint)
  void player.playList(tracks.value, at < 0 ? 0 : at)
}

/** 下载走和搜索页一样的入队流程：核心自己挑音源与音质 */
function download(track: Track) {
  void tasks.enqueue([track])
}

function useKeyword(word: string) {
  // 历史里的词点一下就回搜索页搜，省得再手打
  void router.push({ name: 'search', query: { q: word } })
}

function clearAll() {
  clearRecent()
  clearKeywords()
}
</script>

<template>
  <div class="view">
    <header class="head">
      <div>
        <h1>最近播放</h1>
        <p class="muted">
          {{ tracks.length }} 首<template v-if="totalMs"> · 共 {{ formatDuration(totalMs) }}</template>
          · 只存在本机，点一条就能直接重播
        </p>
      </div>
      <button v-if="tracks.length || keywords.length" class="ghost-btn" type="button" @click="clearAll">
        <Icon name="trash" :size="15" />
        清空记录
      </button>
    </header>

    <section v-if="keywords.length" class="block">
      <div class="block-head">
        <p class="title">最近搜索</p>
        <button class="link-btn" type="button" @click="clearKeywords()">清空</button>
      </div>
      <div class="chips">
        <button
          v-for="word in keywords"
          :key="word"
          class="chip"
          type="button"
          @click="useKeyword(word)"
        >
          {{ word }}
        </button>
      </div>
    </section>

    <section v-if="tracks.length" class="block">
      <div class="block-head">
        <p class="title">播放记录</p>
        <span class="hint">新的在前，最多留 20 首</span>
      </div>
      <div class="rows">
        <TrackRow
          v-for="(track, index) in tracks"
          :key="track.fingerprint"
          :track="track"
          :index="index"
          :active="player.isCurrent(track)"
          :playing="player.playing"
          @play="play"
          @download="download"
        />
      </div>
    </section>

    <div v-else class="empty">
      <span class="empty-icon"><Icon name="music" :size="26" /></span>
      <p class="empty-title">还没有播放记录</p>
      <p class="empty-note">在搜索页点一首歌播一下，这里就会留下记录，下次直接点着重播。</p>
      <button class="primary-btn" type="button" @click="router.push({ name: 'search' })">
        去搜索
      </button>
    </div>
  </div>
</template>

<style scoped>
.view {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 22px 24px 28px;
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

/* 「清空记录」在窄屏下会被挤成两行，和图标叠在一起 */
.head .ghost-btn {
  flex: none;
  white-space: nowrap;
}

h1 {
  margin: 0 0 6px;
  font-size: 22px;
  letter-spacing: -0.01em;
}

.muted {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-2);
}

.ghost-btn,
.link-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: var(--text-2);
  font-size: 12.5px;
  cursor: pointer;
  padding: 6px 8px;
  border-radius: var(--r-sm);
}

.ghost-btn:hover,
.link-btn:hover {
  color: var(--text);
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

.hint {
  font-size: 11.5px;
  color: var(--text-3);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.chip {
  padding: 6px 12px;
  border-radius: var(--r-full);
  border: 1px solid var(--line);
  background: var(--surface);
  color: var(--text-2);
  font-size: 12.5px;
  cursor: pointer;
}

.chip:hover {
  color: var(--text);
  border-color: color-mix(in srgb, var(--accent) 40%, var(--line));
}

.rows {
  border-radius: var(--r-lg);
  background: var(--surface);
  border: 1px solid var(--line);
  overflow: hidden;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 56px 24px;
  text-align: center;
}

.empty-icon {
  display: grid;
  place-items: center;
  width: 60px;
  height: 60px;
  margin-bottom: 6px;
  border-radius: 20px;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  color: var(--accent);
}

.empty-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.empty-note {
  margin: 0;
  max-width: 300px;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-2);
}

.primary-btn {
  margin-top: 10px;
  padding: 9px 20px;
  border: 0;
  border-radius: var(--r-full);
  background: var(--accent);
  color: #fff;
  font-size: 13px;
  cursor: pointer;
}

@media (max-width: 860px) {
  .view {
    padding: 18px 16px 24px;
  }
  h1 {
    font-size: 20px;
  }
}
</style>
