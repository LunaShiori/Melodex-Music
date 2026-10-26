<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import Icon from './Icon.vue'
import { coverOf } from '@/lib/artwork'
import { formatDuration } from '@/lib/format'
import { SOURCE_META } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'

const player = usePlayerStore()

const box = ref<HTMLElement | null>(null)
const rows = ref<HTMLElement[]>([])
const scrubbing = ref(false)
const preview = ref<number | null>(null)

const track = computed(() => player.current)
const cover = computed(() => (track.value ? coverOf(track.value.cover, track.value.fingerprint) : ''))
const sourceMeta = computed(() => (player.variant ? SOURCE_META[player.variant.source] : null))

const shownPosition = computed(() => preview.value ?? player.position)
const shownProgress = computed(() =>
  player.duration ? Math.min(1, Math.max(0, shownPosition.value / player.duration)) : 0,
)

/** 当前行滚到视口中央偏上，让下一句提前进入视线 */
async function centerActive() {
  const index = player.activeLyricIndex
  if (index < 0) return
  await nextTick()
  const row = rows.value[index]
  const wrap = box.value
  if (!row || !wrap) return
  const target = row.offsetTop - wrap.clientHeight * 0.42
  wrap.scrollTo({ top: Math.max(0, target), behavior: 'smooth' })
}

watch(() => player.activeLyricIndex, centerActive)
watch(
  () => player.fullLyric,
  (open) => {
    if (open) void centerActive()
  },
)

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') player.fullLyric = false
  if (event.key === ' ') {
    event.preventDefault()
    player.toggle()
  }
}
onMounted(() => window.addEventListener('keydown', onKeydown))

/* 进度条拖拽 */
function startScrub(event: PointerEvent) {
  const bar = (event.currentTarget as HTMLElement)
  const rect = bar.getBoundingClientRect()
  const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
  preview.value = ratio * player.duration
  const move = (e: PointerEvent) => {
    const r = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width))
    preview.value = r * player.duration
  }
  const up = () => {
    if (preview.value !== null) player.seek(preview.value)
    preview.value = null
    scrubbing.value = false
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  scrubbing.value = true
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}
</script>

<template>
  <Transition name="lyric-sheet">
    <section v-if="player.fullLyric" class="full" @keydown.esc="player.fullLyric = false">
      <!-- 背景氛围：封面放大模糊 + 深色遮罩 -->
      <div class="glow" :style="{ backgroundImage: cover ? `url(${cover})` : 'none' }" />
      <div class="veil" />

      <header class="top">
        <button class="round" type="button" aria-label="收起歌词" @click="player.fullLyric = false">
          <Icon name="chevron" :size="22" />
        </button>
        <div class="top-mid">
          <p class="t-title">{{ track?.title ?? '未在播放' }}</p>
          <p class="t-sub">
            {{ track?.artists.join(' / ') || '未知歌手' }}
            <span v-if="sourceMeta" :style="{ color: sourceMeta.color }">· {{ sourceMeta.short }}</span>
          </p>
        </div>
        <button
          class="round"
          type="button"
          aria-label="关闭"
          @click="player.fullLyric = false"
        >
          <Icon name="close" :size="18" />
        </button>
      </header>

      <div ref="box" class="lyric-scroll">
        <p v-if="!player.lyricLines.length" class="no-lyric">
          <Icon name="lyric" :size="22" />
          这个音源没有提供歌词
        </p>
        <p
          v-for="(line, index) in player.lyricLines"
          :key="index"
          :ref="(el) => { if (el) rows[index] = el as HTMLElement }"
          class="line"
          :class="{ on: index === player.activeLyricIndex }"
          @click="player.seek(line.timeMs)"
        >
          <span class="main">{{ line.text || '♪' }}</span>
          <span v-if="line.translation" class="tr">{{ line.translation }}</span>
        </p>
      </div>

      <footer class="bottom">
        <div class="timeline">
          <span class="t mono">{{ formatDuration(shownPosition) }}</span>
          <div
            class="bar"
            :class="{ scrubbing }"
            @pointerdown.prevent="startScrub"
          >
            <div class="rail" />
            <div class="fill" :style="{ width: `${shownProgress * 100}%` }" />
            <div class="knob" :style="{ left: `${shownProgress * 100}%` }" />
          </div>
          <span class="t mono">{{ formatDuration(player.duration) }}</span>
        </div>

        <div class="controls">
          <button class="ctrl" type="button" :disabled="!track" @click="player.prev()">
            <Icon name="prev" :size="24" />
          </button>
          <button class="big" type="button" :disabled="!track" @click="player.toggle()">
            <Icon
              :name="player.playing ? 'pause' : 'play'"
              :size="24"
              :filled="!player.playing"
            />
          </button>
          <button class="ctrl" type="button" :disabled="!track" @click="player.next()">
            <Icon name="next" :size="24" />
          </button>
        </div>
      </footer>
    </section>
  </Transition>
</template>

<style scoped>
.full {
  position: fixed;
  inset: 0;
  z-index: 55;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  color: var(--text);
}

.glow {
  position: absolute;
  inset: -30%;
  background-size: cover;
  background-position: center;
  filter: blur(72px) saturate(170%);
  opacity: 0.5;
  transform: scale(1.1);
  transition: background-image 400ms var(--ease);
}
.veil {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(120% 70% at 50% 0%, transparent 0%, var(--bg) 82%),
    color-mix(in srgb, var(--bg) 82%, transparent);
  backdrop-filter: blur(28px) saturate(140%);
}

.top,
.lyric-scroll,
.bottom {
  position: relative;
  z-index: 1;
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: calc(env(safe-area-inset-top, 0px) + 14px) 20px 10px;
  flex: 0 0 auto;
}
.round {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  border-radius: 50%;
  color: var(--text-2);
  background: color-mix(in srgb, var(--surface) 44%, transparent);
  border: 1px solid var(--line);
  backdrop-filter: blur(12px);
  transition: transform 140ms var(--ease), color var(--dur) var(--ease);
}
.round:active {
  transform: scale(0.92);
}
.top-mid {
  text-align: center;
  min-width: 0;
  flex: 1;
}
.t-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.t-sub {
  margin: 3px 0 0;
  font-size: 11.5px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lyric-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 8vh 26px 6vh;
  scrollbar-width: none;
  mask-image: linear-gradient(to bottom, transparent, #000 8%, #000 88%, transparent);
}
.lyric-scroll::-webkit-scrollbar {
  display: none;
}
.no-lyric {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  margin: 18vh 0 0;
  font-size: 13px;
  color: var(--text-3);
}

.line {
  margin: 0;
  padding: 14px 0;
  text-align: center;
  cursor: pointer;
  transition: transform 320ms var(--ease), opacity 320ms var(--ease);
  opacity: 0.34;
  transform: scale(0.96);
}
.line .main {
  display: block;
  font-size: 22px;
  line-height: 1.5;
  font-weight: 500;
  letter-spacing: 0.01em;
  color: var(--text-2);
  transition: color 300ms var(--ease);
}
.line .tr {
  display: block;
  margin-top: 5px;
  font-size: 13px;
  color: var(--text-3);
  transition: color 300ms var(--ease);
}
/* 非当前行轻微上移形成景深，当前行放大到前景 */
.line.on {
  opacity: 1;
  transform: scale(1);
}
.line.on .main {
  color: var(--text);
  font-weight: 650;
  /* 卡拉OK式强调：当前行加一道随强调色的下划线光晕 */
  text-shadow: 0 0 22px color-mix(in srgb, var(--accent) 34%, transparent);
}
.line.on .tr {
  color: var(--text-2);
}

.bottom {
  flex: 0 0 auto;
  padding: 12px 24px calc(env(safe-area-inset-bottom, 0px) + 18px);
}
.timeline {
  display: flex;
  align-items: center;
  gap: 10px;
}
.t {
  font-size: 10.5px;
  color: var(--text-3);
  width: 42px;
  flex: 0 0 auto;
}
.t:last-child {
  text-align: right;
}
.bar {
  position: relative;
  flex: 1;
  height: 26px;
  display: flex;
  align-items: center;
  touch-action: none;
}
.rail,
.fill {
  position: absolute;
  left: 0;
  height: 5px;
  border-radius: var(--r-full);
}
.rail {
  right: 0;
  background: color-mix(in srgb, var(--text) 16%, transparent);
}
.fill {
  background: linear-gradient(90deg, var(--accent), var(--accent-soft));
  box-shadow: 0 0 12px -2px color-mix(in srgb, var(--accent) 70%, transparent);
}
.knob {
  position: absolute;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--surface);
  border: 2px solid var(--accent);
  transform: translateX(-50%);
  transition: transform 140ms var(--ease);
}
.bar.scrubbing .knob {
  transform: translateX(-50%) scale(1.3);
}

.controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 30px;
  margin-top: 4px;
}
.ctrl {
  display: grid;
  place-items: center;
  width: 46px;
  height: 46px;
  border-radius: 50%;
  color: var(--text);
  transition: transform 130ms var(--ease), background var(--dur) var(--ease);
}
.ctrl:active {
  transform: scale(0.9);
  background: var(--hover);
}
.ctrl:disabled {
  opacity: 0.35;
}
.big {
  display: grid;
  place-items: center;
  width: 62px;
  height: 62px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--accent-ink);
  box-shadow: 0 14px 30px -12px color-mix(in srgb, var(--accent) 90%, transparent);
  transition: transform 140ms var(--ease);
}
.big:active {
  transform: scale(0.93);
}
.big:disabled {
  opacity: 0.4;
  box-shadow: none;
}

/* 从底部滑入的沉浸式过渡 */
.lyric-sheet-enter-active,
.lyric-sheet-leave-active {
  transition: transform 360ms var(--ease), opacity 300ms var(--ease);
}
.lyric-sheet-enter-from,
.lyric-sheet-leave-to {
  transform: translateY(100%);
  opacity: 0.4;
}

@media (max-width: 640px) {
  .line .main {
    font-size: 19px;
  }
  .line {
    padding: 12px 0;
  }
}
</style>
