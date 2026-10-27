<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import Icon from './Icon.vue'
import { coverOf } from '@/lib/artwork'
import { onCoverError } from '@/lib/cover'
import { formatDuration } from '@/lib/format'
import { SOURCE_META } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'

const player = usePlayerStore()

const track = computed(() => player.current)
const cover = computed(() => (track.value ? coverOf(track.value.cover, track.value.fingerprint) : ''))
const sourceMeta = computed(() => (player.variant ? SOURCE_META[player.variant.source] : null))

const bar = ref<HTMLElement | null>(null)
const scrubbing = ref(false)
const preview = ref<number | null>(null)
const lyricBox = ref<HTMLElement | null>(null)
const lyricRows = ref<HTMLElement[]>([])

/** 拖动时显示拖动位置，松手才真正 seek，避免一直打断缓冲 */
const shownPosition = computed(() => preview.value ?? player.position)
const shownProgress = computed(() =>
  player.duration ? Math.min(1, Math.max(0, shownPosition.value / player.duration)) : 0,
)

function ratioFrom(event: PointerEvent) {
  if (!bar.value) return 0
  const rect = bar.value.getBoundingClientRect()
  return Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
}

function startScrub(event: PointerEvent) {
  scrubbing.value = true
  preview.value = ratioFrom(event) * player.duration
  const move = (e: PointerEvent) => {
    preview.value = ratioFrom(e) * player.duration
  }
  const up = () => {
    if (preview.value !== null) player.seek(preview.value)
    preview.value = null
    scrubbing.value = false
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}

function close() {
  player.expanded = false
}

/** 当前行滚到列表中间，播放时歌词才会自然跟上 */
async function centerActiveLine() {
  const index = player.activeLyricIndex
  if (index < 0) return
  await nextTick()
  const row = lyricRows.value[index]
  const box = lyricBox.value
  if (!row || !box) return
  const target = row.offsetTop - box.clientHeight / 2 + row.clientHeight / 2
  box.scrollTo({ top: Math.max(0, target), behavior: 'smooth' })
}

watch(() => player.activeLyricIndex, centerActiveLine)
watch(
  () => player.expanded,
  (open) => {
    if (open) void centerActiveLine()
  },
)
</script>

<template>
  <Transition name="sheet">
    <section v-if="player.expanded" class="sheet-root" :class="{ open: player.expanded }">
      <div class="glow" :style="{ backgroundImage: cover ? `url(${cover})` : 'none' }" />
      <div class="veil" />

      <header class="top">
        <button class="round" type="button" aria-label="收起" @click="close">
          <Icon name="chevron" :size="20" />
        </button>
        <div class="top-mid">
          <p class="eyebrow">正在播放</p>
          <p v-if="sourceMeta" class="eyebrow src" :style="{ color: sourceMeta.color }">
            {{ sourceMeta.label }}
          </p>
        </div>
        <button
          class="round"
          type="button"
          aria-label="全屏歌词"
          :class="{ on: player.lyricLines.length }"
          @click="player.fullLyric = true"
        >
          <Icon name="lyric" :size="19" />
        </button>
      </header>

      <div class="art">
        <img v-if="cover" :src="cover" :alt="track?.title ?? ''" @error="onCoverError($event, track?.fingerprint ?? 'melodex')" />
        <span v-else class="art-fallback"><Icon name="disc" :size="42" /></span>
        <span v-if="player.loading" class="art-loading"><span class="spin" /></span>
      </div>

      <div class="info">
        <h1>{{ track?.title ?? '未在播放' }}</h1>
        <p>{{ track?.artists.join(' / ') || '未知歌手' }}<template v-if="track?.album"> · {{ track.album }}</template></p>
      </div>

      <div v-if="player.error" class="err">
        <Icon name="alert" :size="15" />
        <div>
          <p class="err-label">{{ player.error.label }} · {{ player.error.message }}</p>
          <p class="err-hint">{{ player.error.hint }}</p>
        </div>
        <button class="btn btn-sm" type="button" @click="player.toggle()">重试</button>
      </div>

      <!-- 歌词优先占位，没有歌词时退回大封面视觉 -->
      <div v-if="player.lyricLines.length" ref="lyricBox" class="lyrics">
        <p
          v-for="(line, index) in player.lyricLines"
          :key="index"
          :ref="(el) => { if (el) lyricRows[index] = el as HTMLElement }"
          class="line"
          :class="{ on: index === player.activeLyricIndex }"
          @click="player.seek(line.timeMs)"
        >
          {{ line.text || '♪' }}
          <span v-if="line.translation" class="tr">{{ line.translation }}</span>
        </p>
      </div>
      <div v-else class="lyrics empty">
        <p class="blank">这个音源没有提供歌词</p>
      </div>

      <div class="bottom">
        <div class="timeline">
          <span class="t mono">{{ formatDuration(shownPosition) }}</span>
          <div
            ref="bar"
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
            <Icon name="prev" :size="26" />
          </button>
          <button class="big" type="button" :disabled="!track" @click="player.toggle()">
            <span v-if="player.loading" class="spin dark" />
            <Icon
              v-else
              :name="player.playing ? 'pause' : 'play'"
              :size="26"
              :filled="!player.playing"
            />
          </button>
          <button class="ctrl" type="button" :disabled="!track" @click="player.next()">
            <Icon name="next" :size="26" />
          </button>
        </div>
      </div>
    </section>
  </Transition>
</template>

<style scoped>
.sheet-root {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  flex-direction: column;
  padding: calc(env(safe-area-inset-top, 0px) + 10px) 22px
    calc(env(safe-area-inset-bottom, 0px) + 18px);
  overflow: hidden;
  color: var(--text);
}

/* 液态玻璃：封面放大成背景，重度模糊压成色块，再叠一层深色遮罩保证文字可读 */
.glow {
  position: absolute;
  inset: -30%;
  background-size: cover;
  background-position: center;
  filter: blur(64px) saturate(160%);
  opacity: 0.55;
  transform: scale(1.1);
  transition: background-image 400ms var(--ease);
}
.veil {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(120% 60% at 50% 0%, transparent 0%, var(--bg) 78%),
    color-mix(in srgb, var(--bg) 74%, transparent);
  backdrop-filter: blur(30px) saturate(140%);
}

.top,
.art,
.info,
.lyrics,
.bottom {
  position: relative;
  z-index: 1;
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex: 0 0 auto;
}
.round {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  color: var(--text-2);
  background: color-mix(in srgb, var(--surface) 42%, transparent);
  border: 1px solid var(--line);
  backdrop-filter: blur(12px);
  transition: transform 140ms var(--ease), color var(--dur) var(--ease);
}
.round:active {
  transform: scale(0.92);
}
.round.on {
  color: var(--accent);
}
.top-mid {
  text-align: center;
  min-width: 0;
}
.eyebrow {
  margin: 0;
  font-size: 10.5px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--text-3);
}
.eyebrow.src {
  margin-top: 2px;
  letter-spacing: 0.04em;
  text-transform: none;
  font-size: 11px;
}

.art {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  margin: 14px 0 0;
}
.art img,
.art-fallback {
  width: min(58vw, 240px);
  aspect-ratio: 1;
  border-radius: 26px;
  object-fit: cover;
  display: block;
  box-shadow: 0 26px 60px -26px rgba(0, 0, 0, 0.7);
}
.art-fallback {
  display: grid;
  place-items: center;
  background: var(--surface-2);
  color: var(--text-3);
}
.art-loading {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  border-radius: 26px;
  background: rgba(0, 0, 0, 0.28);
  backdrop-filter: blur(2px);
}
.spin {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: 2px solid rgba(255, 255, 255, 0.28);
  border-top-color: #fff;
  animation: spin 0.75s linear infinite;
}
.spin.dark {
  border-color: color-mix(in srgb, var(--bg) 30%, transparent);
  border-top-color: var(--bg);
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.info {
  flex: 0 0 auto;
  margin-top: 16px;
  text-align: center;
}
.info h1 {
  margin: 0;
  font-size: 19px;
  font-weight: 600;
  letter-spacing: -0.01em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.info p {
  margin: 5px 0 0;
  font-size: 12.5px;
  color: var(--text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.err {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
  padding: 10px 12px;
  border-radius: var(--r-md);
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  border: 1px solid color-mix(in srgb, var(--danger) 30%, transparent);
  color: var(--text);
}
.err > :first-child {
  color: var(--danger);
  flex: 0 0 auto;
}
.err-label {
  margin: 0;
  font-size: 12px;
  font-weight: 600;
}
.err-hint {
  margin: 2px 0 0;
  font-size: 11px;
  color: var(--text-2);
  line-height: 1.5;
}

.lyrics {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  margin: 14px -6px 0;
  padding: 0 6px 12px;
  scrollbar-width: none;
  /* 上下渐隐，滚动时不会在边缘硬切 */
  mask-image: linear-gradient(to bottom, transparent, #000 12%, #000 82%, transparent);
}
.lyrics::-webkit-scrollbar {
  display: none;
}
.line {
  margin: 0;
  padding: 7px 0;
  font-size: 15px;
  line-height: 1.55;
  text-align: center;
  color: var(--text-3);
  transition: color 220ms var(--ease), transform 220ms var(--ease), opacity 220ms var(--ease);
  transform-origin: center;
}
.line.on {
  color: var(--text);
  font-weight: 600;
  transform: scale(1.06);
}
.tr {
  display: block;
  margin-top: 3px;
  font-size: 12px;
  font-weight: 400;
  color: var(--text-3);
}
.line.on .tr {
  color: var(--text-2);
}
.empty {
  display: grid;
  place-items: center;
}
.blank {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-3);
}

.bottom {
  flex: 0 0 auto;
  margin-top: auto;
  padding-top: 10px;
}
.timeline {
  display: flex;
  align-items: center;
  gap: 10px;
}
.t {
  font-size: 10.5px;
  color: var(--text-3);
  width: 40px;
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
.bar .knob {
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
  transform: translateX(-50%) scale(1.25);
}

.controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 26px;
  margin-top: 6px;
}
.ctrl {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
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
  width: 66px;
  height: 66px;
  border-radius: 50%;
  background: var(--accent);
  color: #fff;
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

/* 抽屉式进出：从底部滑上来，关闭时回落 */
.sheet-enter-active,
.sheet-leave-active {
  transition: transform 340ms var(--ease), opacity 260ms var(--ease);
}
.sheet-enter-from,
.sheet-leave-to {
  transform: translateY(100%);
  opacity: 0.6;
}

/* 桌面端不开这个页面：播放条已经能完成所有操作 */
@media (min-width: 861px) {
  .sheet-root {
    display: none;
  }
}
</style>
