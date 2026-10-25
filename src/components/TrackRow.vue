<script setup lang="ts">
import { computed } from 'vue'

import Icon from './Icon.vue'
import { coverOf } from '@/lib/artwork'
import { formatBitrate, formatDuration } from '@/lib/format'
import { SOURCE_META } from '@/lib/types'
import type { Track, Variant } from '@/lib/types'

const props = defineProps<{ track: Track; index?: number; active?: boolean; playing?: boolean }>()
const emit = defineEmits<{
  (e: 'play', track: Track, variant?: Variant): void
  (e: 'download', track: Track, variant?: Variant): void
}>()

const cover = computed(() => coverOf(props.track.cover, props.track.fingerprint))
const bestBitrate = computed(() => Math.max(...props.track.variants.map((v) => v.bitrate)))
const feeLabel = computed(() => {
  if (props.track.fee === 'vip') return '会员'
  if (props.track.fee === 'paid') return '付费'
  return ''
})
</script>

<template>
  <article class="row" :class="{ active, playing }" @dblclick="emit('play', track)">
    <span class="index mono">
      <template v-if="playing && active">
        <span class="mini-eq"><i /><i /><i /></span>
      </template>
      <template v-else>{{ (index ?? 0) + 1 }}</template>
    </span>

    <div class="cover">
      <img :src="cover" alt="" loading="lazy" />
      <button class="cover-play" type="button" title="播放" @click="emit('play', track)">
        <Icon :name="playing && active ? 'pause' : 'play'" :size="15" :filled="!(playing && active)" />
      </button>
    </div>

    <div class="main">
      <p class="title" :title="track.title">{{ track.title }}</p>
      <p class="sub">
        <span class="artist">{{ track.artists.join(' / ') || '未知歌手' }}</span>
        <span v-if="track.album" class="album">· {{ track.album }}</span>
      </p>
    </div>

    <div class="sources">
      <button
        v-for="variant in track.variants"
        :key="variant.source + variant.id"
        class="src-chip"
        type="button"
        :title="`从${SOURCE_META[variant.source].label}下载`"
        :style="{ '--src': SOURCE_META[variant.source].color }"
        @click="emit('download', track, variant)"
      >
        <i />
        {{ SOURCE_META[variant.source].short }}
      </button>
    </div>

    <div class="quality">
      <span class="bitrate mono">{{ formatBitrate(bestBitrate) }}</span>
      <span v-if="feeLabel" class="fee">{{ feeLabel }}</span>
    </div>

    <span class="dur mono">{{ formatDuration(track.durationMs) }}</span>

    <div class="actions">
      <button class="icon-btn" type="button" title="播放" @click="emit('play', track)">
        <Icon name="play" :size="15" />
      </button>
      <button class="icon-btn dl" type="button" title="下载" @click="emit('download', track)">
        <Icon name="download" :size="15" />
      </button>
    </div>
  </article>
</template>

<style scoped>
.row {
  display: grid;
  grid-template-columns: 24px 44px minmax(0, 1fr) 172px 76px 48px 72px;
  align-items: center;
  gap: 12px;
  height: 60px;
  padding: 0 12px;
  border-radius: var(--r-md);
  transition: background var(--dur) var(--ease);
  position: relative;
}
.row:hover {
  background: var(--hover);
}
.row.active {
  background: var(--active);
}
.row.active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 12px;
  bottom: 12px;
  width: 2px;
  border-radius: 2px;
  background: var(--accent);
}

.index {
  font-size: 11px;
  color: var(--text-3);
  text-align: center;
}

.mini-eq {
  display: inline-flex;
  align-items: flex-end;
  gap: 2px;
  height: 12px;
}
.mini-eq i {
  width: 2px;
  background: var(--accent);
  border-radius: 1px;
  animation: bar 0.9s ease-in-out infinite;
}
.mini-eq i:nth-child(1) {
  height: 60%;
  animation-delay: -0.1s;
}
.mini-eq i:nth-child(2) {
  height: 100%;
  animation-delay: -0.4s;
}
.mini-eq i:nth-child(3) {
  height: 45%;
}
@keyframes bar {
  0%,
  100% {
    transform: scaleY(0.35);
  }
  50% {
    transform: scaleY(1);
  }
}

.cover {
  position: relative;
  width: 44px;
  height: 44px;
  border-radius: 10px;
  overflow: hidden;
  background: var(--surface-2);
}
.cover img {
  width: 100%;
  height: 100%;
  display: block;
  object-fit: cover;
}
.cover-play {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  color: #fff;
  background: rgba(8, 8, 12, 0.55);
  opacity: 0;
  transition: opacity var(--dur) var(--ease);
}
.row:hover .cover-play {
  opacity: 1;
}

.main {
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
.row.active .title {
  color: var(--accent);
}
.sub {
  margin: 1px 0 0;
  font-size: 11.5px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.album {
  opacity: 0.8;
}

.sources {
  display: flex;
  gap: 4px;
  justify-content: flex-start;
}
.src-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 21px;
  padding: 0 7px;
  border-radius: var(--r-full);
  font-size: 10.5px;
  font-weight: 500;
  color: var(--text-2);
  background: var(--surface-2);
  border: 1px solid transparent;
  transition: all var(--dur) var(--ease);
}
.src-chip i {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--src);
}
.src-chip:hover {
  border-color: var(--src);
  color: var(--text);
}

.quality {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}
.bitrate {
  font-size: 10.5px;
  color: var(--text-2);
  padding: 1px 6px;
  border-radius: var(--r-xs);
  background: var(--surface-2);
}
.fee {
  font-size: 9.5px;
  color: var(--warning);
  border: 1px solid color-mix(in srgb, var(--warning) 40%, transparent);
  border-radius: var(--r-xs);
  padding: 0 4px;
}

.dur {
  font-size: 11.5px;
  color: var(--text-3);
  text-align: right;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 2px;
  opacity: 0;
  transition: opacity var(--dur) var(--ease);
}
.row:hover .actions,
.row.active .actions {
  opacity: 1;
}
.actions .icon-btn {
  width: 28px;
  height: 28px;
}
.dl:hover {
  color: var(--accent);
  background: var(--active);
}

@media (max-width: 1100px) {
  .sources {
    max-width: 60px;
    overflow: hidden;
  }
}

@media (hover: none) {
  /* 触屏设备没有 hover，把播放按钮做成封面右下角常驻气浮徽标 */
  .cover-play {
    inset: auto 2px 2px auto;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: rgba(8, 8, 12, 0.66);
    opacity: 1;
  }
}

@media (max-width: 860px) {
  .row {
    grid-template-columns: 38px minmax(0, 1fr) auto;
    gap: 10px;
    height: 64px;
    padding: 0 10px;
  }
  .index,
  .quality,
  .dur,
  .actions {
    display: none;
  }
  .cover {
    width: 38px;
    height: 38px;
  }
  .sources {
    max-width: none;
  }
  .src-chip {
    padding: 0 6px;
  }
}
</style>
