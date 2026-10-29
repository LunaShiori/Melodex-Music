<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'

import Icon from './Icon.vue'
import ScrollText from './ScrollText.vue'
import { coverOf } from '@/lib/artwork'
import { onCoverError } from '@/lib/cover'
import { formatDuration } from '@/lib/format'
import { SOURCE_META } from '@/lib/types'
import { useLyricStore } from '@/stores/lyric'
import { usePlayerStore } from '@/stores/player'

const player = usePlayerStore()
const lyric = useLyricStore()
const bar = ref<HTMLElement | null>(null)
const scrubbing = ref(false)
/** 鼠标停在音量区时显示百分比；移开就收起来，避免一直占位 */
const volumeHot = ref(false)

const MODE_TITLE: Record<string, string> = {
  order: '顺序播放',
  'repeat-one': '单曲循环',
  shuffle: '随机播放',
}
const modeTitle = computed(() => `${MODE_TITLE[player.mode] ?? '顺序播放'}（点击切换）`)

/** 滚轮一格 5%：往下滚调小，往上滚调大 */
function onVolumeWheel(event: WheelEvent) {
  player.setVolume(player.volume + (event.deltaY < 0 ? 0.05 : -0.05))
}

const cover = computed(() =>
  player.current ? coverOf(player.current.cover, player.current.fingerprint) : '',
)
const sourceMeta = computed(() =>
  player.variant ? SOURCE_META[player.variant.source] : null,
)

/**
 * 没进歌词详情页时，播放条上要能看见「现在唱到哪句」。
 * 桌面端横排，歌词挂在歌名右边；手机上宽度不够，放到第二行（原来是歌手名）。
 */
const isNarrow = ref(false)
function syncWidth() {
  isNarrow.value = window.matchMedia('(max-width: 860px)').matches
}
onMounted(() => {
  syncWidth()
  window.addEventListener('resize', syncWidth)
})
onUnmounted(() => window.removeEventListener('resize', syncWidth))

/** 详情页开着、没在播、或者这首压根没歌词时都不显示 */
const barLyric = computed(() => {
  if (!lyric.showInBar || player.detailOpen) return ''
  return player.currentLyric
})

function seekFromEvent(event: PointerEvent) {
  if (!bar.value || !player.duration) return
  const rect = bar.value.getBoundingClientRect()
  const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
  player.seek(ratio * player.duration)
}

function startScrub(event: PointerEvent) {
  scrubbing.value = true
  seekFromEvent(event)
  const move = (e: PointerEvent) => seekFromEvent(e)
  const up = () => {
    scrubbing.value = false
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}

/** 点封面 / 播放条信息区 → 歌词详情页。两端同一个入口 */
function openDetail() {
  if (!player.current) return
  player.detailOpen = true
}
</script>

<template>
  <footer class="player" :class="{ empty: !player.current }">
    <!-- 失败原因：说清为什么播不了，而不是静默无声 -->
    <Transition name="slide-up">
      <div v-if="player.error" class="play-error" role="alert">
        <Icon name="alert" :size="15" />
        <div class="pe-text">
          <p class="pe-title">{{ player.error.label }}</p>
          <p class="pe-hint">{{ player.error.hint }}</p>
        </div>
        <button class="btn btn-sm" type="button" @click="player.toggle()">重试</button>
      </div>
    </Transition>

    <div class="mobile-progress">
      <div class="mobile-fill" :style="{ width: `${player.progress * 100}%` }" />
    </div>

    <div class="now" @click="openDetail">
      <div class="thumb" :title="player.current ? '打开歌词详情' : ''">
        <img v-if="cover" :src="cover" alt="" @error="onCoverError($event, player.current?.fingerprint ?? 'melodex')" />
        <span v-else class="placeholder"><Icon name="disc" :size="20" /></span>
        <span v-if="player.playing" class="eq" aria-hidden="true">
          <i /><i /><i />
        </span>
        <span class="thumb-hint" aria-hidden="true"><Icon name="lyric" :size="14" /></span>
      </div>
      <div class="meta">
        <p class="title-row">
          <span class="title">{{ player.current?.title ?? '未在播放' }}</span>
          <Transition v-if="!isNarrow" name="lyric-swap" mode="out-in">
            <span v-if="barLyric" :key="player.activeLyricIndex" class="bar-lyric">
              <ScrollText :text="barLyric" />
            </span>
          </Transition>
        </p>
        <p class="sub">
          <Transition v-if="isNarrow" name="lyric-swap" mode="out-in">
            <span v-if="barLyric" :key="player.activeLyricIndex" class="sub-lyric">
              <ScrollText :text="barLyric" />
            </span>
            <template v-else>
              <span :key="'meta'">
                {{ player.current ? player.current.artists.join(' / ') || '未知歌手' : '从搜索结果里点一下播放' }}
                <span v-if="sourceMeta" class="src" :style="{ color: sourceMeta.color }">
                  · {{ sourceMeta.short }}
                </span>
              </span>
            </template>
          </Transition>
          <template v-else>
            {{ player.current ? player.current.artists.join(' / ') || '未知歌手' : '从搜索结果里点一下播放' }}
            <span v-if="sourceMeta" class="src" :style="{ color: sourceMeta.color }">
              · {{ sourceMeta.short }}
            </span>
          </template>
        </p>
      </div>
    </div>

    <div class="center">
      <div class="controls">
        <button class="icon-btn" title="上一首" :disabled="!player.current" @click="player.prev()">
          <Icon name="prev" :size="18" />
        </button>
        <button
          class="play"
          :class="{ busy: player.loading }"
          :disabled="!player.current"
          @click="player.toggle()"
        >
          <span v-if="player.loading" class="spinner" />
          <Icon
            v-else
            :name="player.playing ? 'pause' : 'play'"
            :size="18"
            :filled="!player.playing"
          />
        </button>
        <button class="icon-btn" title="下一首" :disabled="!player.current" @click="player.next()">
          <Icon name="next" :size="18" />
        </button>
        <button
          class="icon-btn mode-btn"
          :class="{ 'is-active': player.mode !== 'order' }"
          :title="modeTitle"
          @click="player.cycleMode()"
        >
          <Icon :name="player.mode === 'repeat-one' ? 'repeat' : player.mode === 'shuffle' ? 'shuffle' : 'order'" :size="17" />
        </button>
      </div>
      <div class="timeline">
        <span class="t mono">{{ formatDuration(player.position) }}</span>
        <div ref="bar" class="bar" :class="{ scrubbing }" @pointerdown.prevent="startScrub">
          <div class="fill" :style="{ width: `${player.progress * 100}%` }" />
          <div class="knob" :style="{ left: `${player.progress * 100}%` }" />
        </div>
        <span class="t mono">{{ formatDuration(player.duration) }}</span>
      </div>
    </div>

    <div class="right">
      <button
        class="icon-btn"
        :class="{ 'is-active': player.queueOpen }"
        title="播放队列"
        :disabled="!player.queue.length"
        @click="player.toggleQueue()"
      >
        <Icon name="queue" :size="17" />
      </button>
      <button
        class="icon-btn"
        :class="{ 'is-active': player.detailOpen }"
        title="歌词详情"
        :disabled="!player.current"
        @click="openDetail"
      >
        <Icon name="lyric" :size="17" />
      </button>
      <div
        class="volume"
        :class="{ hot: volumeHot }"
        @mouseenter="volumeHot = true"
        @mouseleave="volumeHot = false"
        @wheel.prevent="onVolumeWheel"
      >
        <button class="icon-btn" title="静音" @click="player.setVolume(player.volume > 0 ? 0 : 0.8)">
          <Icon :name="player.volume > 0 ? 'volume' : 'mute'" :size="17" />
        </button>
        <input
          class="slider"
          type="range"
          min="0"
          max="1"
          step="0.01"
          :value="player.volume"
          :title="`音量 ${Math.round(player.volume * 100)}%`"
          @input="player.setVolume(Number(($event.target as HTMLInputElement).value))"
        />
        <span v-if="volumeHot" class="volume-pct mono">{{ Math.round(player.volume * 100) }}%</span>
      </div>
    </div>

    <!-- 手机端的常驻控制：与控制台三键同一套逻辑，只是位置不同 -->
    <div class="mobile-controls">
      <button class="m-btn" :disabled="!player.current" @click="player.prev()">
        <Icon name="prev" :size="20" />
      </button>
      <button class="m-play" :disabled="!player.current" @click="player.toggle()">
        <span v-if="player.loading" class="spinner dark" />
        <Icon
          v-else
          :name="player.playing ? 'pause' : 'play'"
          :size="20"
          :filled="!player.playing"
        />
      </button>
      <button class="m-btn" :disabled="!player.current" @click="player.next()">
        <Icon name="next" :size="20" />
      </button>
      <button
        class="m-btn"
        :class="{ 'is-active': player.queueOpen }"
        :disabled="!player.queue.length"
        title="播放队列"
        @click="player.toggleQueue()"
      >
        <Icon name="queue" :size="18" />
      </button>
    </div>
  </footer>
</template>

<style scoped>
.player {
  flex: 0 0 auto;
  height: var(--player);
  display: grid;
  grid-template-columns: minmax(220px, 1.1fr) minmax(320px, 1.4fr) minmax(160px, 1fr);
  align-items: center;
  gap: 18px;
  padding: 0 18px;
  border-top: 1px solid var(--line);
  background: var(--glass);
  backdrop-filter: blur(18px) saturate(1.4);
  position: relative;
  z-index: 20;
}

/* 失败原因条：贴在播放条顶边，不挤压布局 */
.play-error {
  position: absolute;
  left: 12px;
  right: 12px;
  bottom: calc(100% + 8px);
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: var(--r-md);
  border: 1px solid color-mix(in srgb, var(--danger) 34%, transparent);
  background: color-mix(in srgb, var(--danger) 12%, var(--surface));
  color: var(--text);
  box-shadow: var(--shadow-md);
  z-index: 2;
}
.play-error > :first-child {
  color: var(--danger);
  flex: 0 0 auto;
}
.pe-text {
  min-width: 0;
  flex: 1;
}
.pe-title {
  margin: 0;
  font-size: 12.5px;
  font-weight: 600;
}
.pe-hint {
  margin: 2px 0 0;
  font-size: 11.5px;
  color: var(--text-2);
  line-height: 1.5;
}

.now {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  cursor: pointer;
}

.thumb {
  position: relative;
  width: 50px;
  height: 50px;
  border-radius: 12px;
  overflow: hidden;
  flex: 0 0 auto;
  background: var(--surface-2);
  box-shadow: var(--shadow-sm);
}
.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  transition: transform 220ms var(--ease);
}
.now:hover .thumb img {
  transform: scale(1.06);
}
/* 悬停时给个「点我进歌词」的暗示 */
.thumb-hint {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  color: #fff;
  background: rgba(0, 0, 0, 0.42);
  opacity: 0;
  transition: opacity 180ms var(--ease);
}
.now:hover .thumb-hint {
  opacity: 1;
}
.placeholder {
  display: grid;
  place-items: center;
  height: 100%;
  color: var(--text-3);
}

.eq {
  position: absolute;
  inset: auto 0 0 0;
  height: 16px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  gap: 2px;
  padding-bottom: 3px;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.45), transparent);
}
.eq i {
  width: 2px;
  border-radius: 1px;
  background: #fff;
  animation: bounce 0.9s ease-in-out infinite;
}
.eq i:nth-child(1) {
  height: 40%;
  animation-delay: -0.2s;
}
.eq i:nth-child(2) {
  height: 80%;
  animation-delay: -0.5s;
}
.eq i:nth-child(3) {
  height: 55%;
}
@keyframes bounce {
  0%,
  100% {
    transform: scaleY(0.4);
  }
  50% {
    transform: scaleY(1);
  }
}

.meta {
  min-width: 0;
  flex: 1;
}
.title-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: 0;
  min-width: 0;
}
.title {
  /* 不许被歌词挤扁：先用内容宽度，超过 58% 才省略 */
  flex: 0 0 auto;
  min-width: 0;
  max-width: 58%;
  font-size: 13.5px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
/* 歌名右边那句当前歌词：压暗、跑马灯，和歌名区分开 */
.bar-lyric {
  position: relative;
  /* 必须是 flex 而不是 inline：里面的滚动文本是块级，行内父元素会被它撑成两行 */
  display: flex;
  align-items: baseline;
  flex: 1 1 auto;
  min-width: 0;
  font-size: 12px;
  color: var(--text-3);
  -webkit-mask-image: linear-gradient(90deg, #000 0, #000 88%, transparent);
  mask-image: linear-gradient(90deg, #000 0, #000 88%, transparent);
}
.bar-lyric::before {
  content: '';
  flex: 0 0 auto;
  width: 3px;
  height: 3px;
  margin-right: 7px;
  border-radius: 50%;
  background: currentColor;
  vertical-align: 2px;
  opacity: 0.7;
}
.sub {
  margin: 2px 0 0;
  font-size: 11.5px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sub-lyric {
  display: block;
  color: var(--text-2);
  -webkit-mask-image: linear-gradient(90deg, #000 0, #000 86%, transparent);
  mask-image: linear-gradient(90deg, #000 0, #000 86%, transparent);
}
.src {
  font-weight: 500;
}

/* 歌词换行时轻微上滑淡入，不打断视线 */
.lyric-swap-enter-active,
.lyric-swap-leave-active {
  transition: opacity 220ms var(--ease), transform 220ms var(--ease);
}
.lyric-swap-enter-from {
  opacity: 0;
  transform: translateY(5px);
}
.lyric-swap-leave-to {
  opacity: 0;
  transform: translateY(-5px);
}

.center {
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: center;
}

.controls {
  display: flex;
  align-items: center;
  gap: 6px;
}
.play {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: var(--text);
  color: var(--bg);
  transition: transform var(--dur) var(--ease), opacity var(--dur) var(--ease);
}
.play:hover {
  transform: scale(1.06);
}
.play:active {
  transform: scale(0.96);
}
.play:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.spinner {
  width: 15px;
  height: 15px;
  border-radius: 50%;
  border: 2px solid color-mix(in srgb, var(--bg) 28%, transparent);
  border-top-color: var(--bg);
  animation: spin 0.7s linear infinite;
}
.spinner.dark {
  border-color: color-mix(in srgb, #fff 30%, transparent);
  border-top-color: #fff;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.timeline {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
}
.t {
  font-size: 10.5px;
  color: var(--text-3);
  width: 38px;
  text-align: center;
  flex: 0 0 auto;
}
.bar {
  position: relative;
  flex: 1;
  height: 18px;
  display: flex;
  align-items: center;
  cursor: pointer;
  touch-action: none;
}
/* 轨道用「文字色混一点底」，浅色主题下才是可见的灰，而不是和卡片同色的白 */
.bar::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  height: 5px;
  border-radius: var(--r-full);
  background: color-mix(in srgb, var(--text) 22%, transparent);
}
.fill {
  position: absolute;
  left: 0;
  height: 5px;
  border-radius: var(--r-full);
  background: linear-gradient(90deg, var(--accent), var(--accent-soft));
}
.knob {
  position: absolute;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: var(--surface);
  border: 2px solid var(--accent);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.28);
  transform: translateX(-50%);
  opacity: 0;
  transition: opacity var(--dur) var(--ease);
}
.bar:hover .knob,
.bar.scrubbing .knob {
  opacity: 1;
}

.right {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}
.volume {
  position: relative;
  display: flex;
  align-items: center;
  gap: 4px;
}
/* 百分比浮在音量条上方，不挤占布局（悬停才出现） */
.volume-pct {
  position: absolute;
  left: 50%;
  bottom: calc(100% + 6px);
  transform: translateX(-50%);
  padding: 2px 7px;
  border-radius: var(--r-full);
  background: var(--text);
  color: var(--bg);
  font-size: 11px;
  line-height: 1.4;
  white-space: nowrap;
  pointer-events: none;
  animation: pct-in 120ms ease-out;
}
@keyframes pct-in {
  from {
    opacity: 0;
    transform: translate(-50%, 3px);
  }
}
.mode-btn.is-active {
  color: var(--accent);
}
.slider {
  width: 76px;
  height: 5px;
  appearance: none;
  border-radius: var(--r-full);
  /* 同进度条：浅色主题下不能再拿 surface-3 当轨道，那样几乎看不见 */
  background: color-mix(in srgb, var(--text) 22%, transparent);
  outline: none;
}
.slider::-webkit-slider-thumb {
  appearance: none;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: var(--text);
  box-shadow: 0 0 0 2px var(--surface);
  cursor: pointer;
}
.slider::-moz-range-thumb {
  width: 11px;
  height: 11px;
  border: none;
  border-radius: 50%;
  background: var(--text);
  box-shadow: 0 0 0 2px var(--surface);
  cursor: pointer;
}

.mobile-progress {
  display: none;
}
.mobile-controls {
  display: none;
}

.empty .title {
  color: var(--text-3);
}

/* ---------- 手机端：悬浮玻璃卡 + 常驻控制 ---------- */
@media (max-width: 860px) {
  .player {
    height: auto;
    /* 左右缩进跟底部导航条对齐（14px），两块卡片看起来是一套的 */
    margin: 0 14px 8px;
    padding: 9px 12px;
    border: 1px solid var(--line);
    border-radius: 24px;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    background: color-mix(in srgb, var(--surface) 62%, transparent);
    backdrop-filter: blur(28px) saturate(1.8);
    box-shadow: 0 10px 30px -12px rgba(0, 0, 0, 0.5);
  }
  .center,
  .right {
    display: none;
  }
  .now {
    gap: 10px;
    min-width: 0;
  }
  .thumb {
    width: 40px;
    height: 40px;
    border-radius: 11px;
  }
  .title {
    max-width: 100%;
    font-size: 13px;
  }
  .sub {
    font-size: 11px;
  }
  .thumb-hint {
    display: none;
  }

  .mobile-controls {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .m-btn {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    color: var(--text-2);
    transition: background var(--dur) var(--ease), transform 120ms var(--ease);
  }
  .m-btn:active {
    transform: scale(0.9);
    background: var(--hover);
  }
  .m-btn:disabled {
    opacity: 0.35;
  }
  .m-play {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    box-shadow: 0 6px 16px -6px color-mix(in srgb, var(--accent) 80%, transparent);
    transition: transform 140ms var(--ease);
  }
  .m-play:active {
    transform: scale(0.92);
  }
  .m-play:disabled {
    opacity: 0.4;
    box-shadow: none;
  }

  .mobile-progress {
    display: block;
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: 3px;
    height: 2px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 20%, transparent);
    overflow: hidden;
  }
  .mobile-fill {
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 240ms linear;
  }

  .play-error {
    position: static;
    grid-column: 1 / -1;
    margin-bottom: 2px;
    box-shadow: none;
  }
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: opacity 200ms var(--ease), transform 200ms var(--ease);
}
.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
</style>
