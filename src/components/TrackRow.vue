<script setup lang="ts">
import { computed } from 'vue'

import Icon from './Icon.vue'
import { coverOf } from '@/lib/artwork'
import { onCoverError } from '@/lib/cover'
import { formatBitrate, formatDuration } from '@/lib/format'
import { originLabel } from '@/lib/origin'
import { SOURCE_META } from '@/lib/types'
import type { Track, Variant } from '@/lib/types'

const props = defineProps<{
  track: Track
  index?: number
  active?: boolean
  playing?: boolean
  /** 多选模式下的勾选状态 */
  selected?: boolean
  /** 多选模式：整行点击改为勾选，不再触发播放 */
  selectMode?: boolean
}>()
const emit = defineEmits<{
  (e: 'play', track: Track, variant?: Variant): void
  (e: 'download', track: Track, variant?: Variant): void
  (e: 'toggle', track: Track): void
}>()

const cover = computed(() => coverOf(props.track.cover, props.track.fingerprint))
const bestBitrate = computed(() => Math.max(...props.track.variants.map((v) => v.bitrate)))
/** 标题自带「原唱 / 翻唱」说明时给个小标签，扫一眼就知道这条是什么版本 */
const origin = computed(() => originLabel(props.track))
const feeLabel = computed(() => {
  if (props.track.fee === 'vip') return '会员'
  if (props.track.fee === 'paid') return '付费'
  return ''
})

function onRowClick() {
  if (props.selectMode) {
    emit('toggle', props.track)
    return
  }
  // 窄屏没有 hover、也没有双击习惯，点一下整行就是播放入口
  if (isNarrow()) emit('play', props.track)
}

function isNarrow() {
  return typeof window !== 'undefined' && window.matchMedia('(max-width: 860px)').matches
}

function onPlay() {
  if (props.selectMode) {
    emit('toggle', props.track)
    return
  }
  emit('play', props.track)
}
</script>

<template>
  <article
    class="row"
    :class="{ active, playing, picked: selected, selectable: selectMode }"
    @dblclick="!selectMode && emit('play', track)"
    @click="onRowClick"
  >
    <!-- 勾选框只在多选模式出现；平时这个位置是序号 -->
    <span v-if="selectMode" class="pick">
      <span class="box" :class="{ on: selected }">
        <Icon v-if="selected" name="check" :size="12" />
      </span>
    </span>
    <span v-else class="index mono">
      <template v-if="playing && active">
        <span class="mini-eq"><i /><i /><i /></span>
      </template>
      <template v-else>{{ (index ?? 0) + 1 }}</template>
    </span>

    <div class="cover" @click.stop="onPlay">
      <img :src="cover" alt="" loading="lazy" @error="onCoverError($event, track.fingerprint)" />
      <button class="cover-play" type="button" :title="playing && active ? '暂停' : '播放'">
        <Icon
          :name="playing && active ? 'pause' : 'play'"
          :size="15"
          :filled="!(playing && active)"
        />
      </button>
    </div>

    <div class="main">
      <p class="title" :title="track.title">
        <span
          v-if="origin"
          class="origin-tag"
          :class="origin === '原唱' ? 'tag-orig' : 'tag-cover'"
        >{{ origin }}</span>
        <span class="title-text">{{ track.title }}</span>
      </p>
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
        @click.stop="emit('download', track, variant)"
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
      <button class="icon-btn" type="button" title="播放" @click.stop="emit('play', track)">
        <Icon name="play" :size="15" />
      </button>
      <button class="icon-btn dl" type="button" title="下载" @click.stop="emit('download', track)">
        <Icon name="download" :size="15" />
      </button>
    </div>

    <!-- 手机端没有 hover，下载入口必须常驻可见 -->
    <button class="m-dl" type="button" title="下载" @click.stop="emit('download', track)">
      <Icon name="download" :size="17" />
    </button>
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
.row.picked {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}
.row.selectable {
  cursor: pointer;
}

.index {
  font-size: 11px;
  color: var(--text-3);
  text-align: center;
}

.pick {
  display: grid;
  place-items: center;
}
.box {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border-radius: 6px;
  border: 1.5px solid var(--line-strong);
  color: #fff;
  transition: background 140ms var(--ease), border-color 140ms var(--ease), transform 140ms var(--ease);
}
.box.on {
  background: var(--accent);
  border-color: var(--accent);
  transform: scale(1.05);
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
  /* 标签和歌名各占一个 flex item：标签永远不会被挤到下一行，
     窄屏/大字号的手机上也不会出现「标签独占一行、歌名缩到下面」的错位 */
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 0;
  font-size: 13.5px;
  font-weight: 500;
  min-width: 0;
}
.title-text {
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.row.active .title {
  color: var(--accent);
}
.origin-tag {
  /* 高度写死，不跟着行高/字号放大跑，任何字体环境下都是同一颗小药丸 */
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  height: 16px;
  padding: 0 5px;
  border-radius: 5px;
  font-size: 10px;
  font-weight: 600;
  line-height: 1;
  letter-spacing: 0.02em;
  white-space: nowrap;
}
/* 修饰类名必须和封面图的 .cover 错开：改成 .cover 会让标签吃到封面的 40×40 尺寸，
   标签被撑成方块、歌名被挤到下一行——这就是之前手机端错位的原因 */
.origin-tag.tag-orig {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
}
.origin-tag.tag-cover {
  color: var(--text-3);
  background: var(--surface-2);
  border: 1px solid var(--line);
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

.m-dl {
  display: none;
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
    grid-template-columns: 40px minmax(0, 1fr) auto auto;
    gap: 10px;
    height: 62px;
    padding: 0 10px;
  }
  /* 触屏没有 hover，按下要有即时反馈 */
  .row:active {
    background: var(--hover);
  }
  .row.picked:active {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .row.selectable {
    grid-template-columns: 24px 40px minmax(0, 1fr) auto;
  }
  .index,
  .quality,
  .dur,
  .actions,
  .sources {
    display: none;
  }
  .cover {
    width: 40px;
    height: 40px;
    border-radius: 9px;
  }
  .sub {
    font-size: 11px;
  }
  .m-dl {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    color: var(--text-2);
    background: color-mix(in srgb, var(--surface) 60%, transparent);
    border: 1px solid var(--line);
    transition: transform 130ms var(--ease), color var(--dur) var(--ease);
  }
  .m-dl:active {
    transform: scale(0.9);
    color: var(--accent);
  }
}
</style>
