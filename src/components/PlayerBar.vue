<script setup lang="ts">
import { computed, ref } from 'vue'

import Icon from './Icon.vue'
import { coverOf } from '@/lib/artwork'
import { formatDuration } from '@/lib/format'
import { SOURCE_META } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'

const player = usePlayerStore()
const bar = ref<HTMLElement | null>(null)
const scrubbing = ref(false)

const cover = computed(() =>
  player.current ? coverOf(player.current.cover, player.current.fingerprint) : '',
)
const sourceMeta = computed(() =>
  player.variant ? SOURCE_META[player.variant.source] : null,
)

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

/** 手机端点播放条 → 全屏播放页；桌面端播放条本身已经够用 */
function openSheet() {
  if (!player.current) return
  if (window.matchMedia('(max-width: 860px)').matches) player.expanded = true
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

    <div class="now" @click="openSheet">
      <div class="thumb">
        <img v-if="cover" :src="cover" alt="" />
        <span v-else class="placeholder"><Icon name="disc" :size="20" /></span>
        <span v-if="player.playing" class="eq" aria-hidden="true">
          <i /><i /><i />
        </span>
      </div>
      <div class="meta">
        <p class="title">{{ player.current?.title ?? '未在播放' }}</p>
        <p class="sub">
          <template v-if="player.current">
            {{ player.current.artists.join(' / ') || '未知歌手' }}
            <span v-if="sourceMeta" class="src" :style="{ color: sourceMeta.color }">
              · {{ sourceMeta.short }}
            </span>
          </template>
          <template v-else>从搜索结果里点一下播放</template>
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
        :class="{ 'is-active': player.lyricOpen }"
        title="歌词"
        @click="player.lyricOpen = !player.lyricOpen"
      >
        <Icon name="lyric" :size="17" />
      </button>
      <div class="volume">
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
          @input="player.setVolume(Number(($event.target as HTMLInputElement).value))"
        />
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
    </div>
  </footer>
</template>

<style scoped>
.player {
  flex: 0 0 auto;
  height: var(--player);
  display: grid;
  grid-template-columns: minmax(200px, 1fr) minmax(320px, 1.4fr) minmax(160px, 1fr);
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
}
.title {
  margin: 0;
  font-size: 13.5px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sub {
  margin: 2px 0 0;
  font-size: 11.5px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.src {
  font-weight: 500;
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
.bar::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  height: 4px;
  border-radius: var(--r-full);
  background: var(--surface-3);
}
.fill {
  position: absolute;
  left: 0;
  height: 4px;
  border-radius: var(--r-full);
  background: linear-gradient(90deg, var(--accent), var(--accent-soft));
}
.knob {
  position: absolute;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--surface);
  border: 2px solid var(--accent);
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
  display: flex;
  align-items: center;
  gap: 4px;
}
.slider {
  width: 76px;
  height: 4px;
  appearance: none;
  border-radius: var(--r-full);
  background: var(--surface-3);
  outline: none;
}
.slider::-webkit-slider-thumb {
  appearance: none;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--text);
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
    margin: 0 10px 8px;
    padding: 9px 12px;
    border: 1px solid var(--line);
    border-radius: 20px;
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
    font-size: 13px;
  }
  .sub {
    font-size: 11px;
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
    background: color-mix(in srgb, var(--text) 12%, transparent);
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
