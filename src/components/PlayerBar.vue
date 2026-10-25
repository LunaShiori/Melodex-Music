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
</script>

<template>
  <footer class="player" :class="{ empty: !player.current }">
    <div class="mobile-progress">
      <div class="mobile-fill" :style="{ width: `${player.progress * 100}%` }" />
    </div>
    <div class="now">
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
        <button class="play" :disabled="!player.current" @click="player.toggle()">
          <Icon :name="player.playing ? 'pause' : 'play'" :size="18" :filled="!player.playing" />
        </button>
        <button class="icon-btn" title="下一首" :disabled="!player.current" @click="player.next()">
          <Icon name="next" :size="18" />
        </button>
      </div>
      <div class="timeline">
        <span class="t mono">{{ formatDuration(player.position) }}</span>
        <div
          ref="bar"
          class="bar"
          :class="{ scrubbing }"
          @pointerdown.prevent="startScrub"
        >
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

.empty .title {
  color: var(--text-3);
}

@media (max-width: 860px) {
  .player {
    height: 64px;
    grid-template-columns: 1fr auto;
    gap: 10px;
    padding: 0 12px calc(env(safe-area-inset-bottom, 0px) * 0.2);
  }
  .center {
    display: none;
  }
  .right .volume {
    display: none;
  }
  .thumb {
    width: 42px;
    height: 42px;
  }
  .mobile-progress {
    display: block;
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    background: var(--surface-3);
  }
  .mobile-fill {
    height: 100%;
    background: var(--accent);
    transition: width 240ms linear;
  }
}
</style>
