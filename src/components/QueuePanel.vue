<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'

import Icon from './Icon.vue'
import { coverOf } from '@/lib/artwork'
import { onCoverError } from '@/lib/cover'
import { usePlayerStore } from '@/stores/player'

/**
 * 播放队列抽屉。
 *
 * 队列本来就在播放器里（`playList` 写进去、上一首/下一首从里面取），
 * 只是一直没有地方看得见。手机上从底部升起，桌面上从右侧滑出，
 * 两者共用同一份 DOM：拖动排序在桌面端用原生拖放，触屏则靠每行的上下箭头。
 */
const player = usePlayerStore()

const dragFrom = ref(-1)

function onDragStart(index: number) {
  dragFrom.value = index
}

function onDrop(index: number) {
  if (dragFrom.value >= 0) player.move(dragFrom.value, index)
  dragFrom.value = -1
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') player.closeQueue()
}

onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <Transition name="queue">
      <div v-if="player.queueOpen" class="q-mask" @click.self="player.closeQueue()">
        <aside class="q-panel" role="dialog" aria-label="播放队列">
          <header class="q-head">
            <div class="q-title">
              <strong>播放队列</strong>
              <span class="q-count mono">{{ player.queue.length }} 首</span>
            </div>
            <button class="q-close" title="关闭" @click="player.closeQueue()">
              <Icon name="close" :size="16" />
            </button>
          </header>

          <p v-if="!player.queue.length" class="q-empty">
            还没有队列。从搜索结果或最近播放里点一首，接下来的歌就会排在这里。
          </p>

          <ol v-else class="q-list">
            <li
              v-for="(track, index) in player.queue"
              :key="`${track.fingerprint}-${index}`"
              class="q-row"
              :class="{ active: index === player.index, dragging: dragFrom === index }"
              draggable="true"
              @dragstart="onDragStart(index)"
              @dragend="dragFrom = -1"
              @dragover.prevent
              @drop="onDrop(index)"
            >
              <span class="q-index mono">{{ String(index + 1).padStart(2, '0') }}</span>
              <button class="q-main" type="button" @click="player.jumpTo(index)">
                <img
                  class="q-cover"
                  :src="coverOf(track.cover, track.fingerprint)"
                  alt=""
                  loading="lazy"
                  @error="onCoverError($event, track.fingerprint)"
                />
                <span class="q-text">
                  <span class="q-name">{{ track.title }}</span>
                  <span class="q-artist">{{ track.artists.join(' / ') || '未知歌手' }}</span>
                </span>
                <span v-if="index === player.index" class="q-live" aria-hidden="true">
                  <i /><i /><i />
                </span>
              </button>
              <span class="q-actions">
                <button
                  class="q-btn"
                  type="button"
                  title="上移"
                  :disabled="index === 0"
                  @click.stop="player.move(index, index - 1)"
                >
                  <Icon name="chevron" :size="13" class="up" />
                </button>
                <button
                  class="q-btn"
                  type="button"
                  title="下移"
                  :disabled="index === player.queue.length - 1"
                  @click.stop="player.move(index, index + 1)"
                >
                  <Icon name="chevron" :size="13" class="down" />
                </button>
                <button class="q-btn" type="button" title="移出队列" @click.stop="player.removeAt(index)">
                  <Icon name="close" :size="13" />
                </button>
              </span>
            </li>
          </ol>
        </aside>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.q-mask {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  background: rgba(12, 12, 18, 0.36);
  backdrop-filter: blur(2px);
}

.q-panel {
  width: min(560px, 100%);
  max-height: 72vh;
  display: flex;
  flex-direction: column;
  /* 液态玻璃主题下 --surface 是半透明的，直接铺会让底下的播放条透出来，
     所以先垫一层不透明的 --bg，再把 --surface 叠上去保持同款色调 */
  background: linear-gradient(var(--surface), var(--surface)), var(--bg);
  backdrop-filter: blur(24px) saturate(1.3);
  border-radius: 22px 22px 0 0;
  box-shadow: 0 -18px 50px -20px rgba(0, 0, 0, 0.45);
  overflow: hidden;
}

.q-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px 10px;
  border-bottom: 1px solid var(--line);
}
.q-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 15px;
}
.q-count {
  font-size: 12px;
  color: var(--text-2);
}
.q-close {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  color: var(--text-2);
}
.q-close:hover {
  background: var(--hover);
  color: var(--text);
}

.q-empty {
  margin: 0;
  padding: 26px 18px 34px;
  font-size: 13px;
  color: var(--text-2);
  text-align: center;
}

.q-list {
  list-style: none;
  margin: 0;
  padding: 6px 8px 14px;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.q-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: 12px;
}
.q-row:hover {
  background: var(--hover);
}
.q-row.active {
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}
.q-row.dragging {
  opacity: 0.45;
}

.q-index {
  width: 20px;
  flex: none;
  font-size: 11px;
  color: var(--text-3);
  text-align: center;
}

.q-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 0;
  background: none;
  border: 0;
  color: inherit;
  text-align: left;
  cursor: pointer;
}
.q-cover {
  width: 34px;
  height: 34px;
  border-radius: 8px;
  object-fit: cover;
  flex: none;
}
.q-text {
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.q-name {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.q-row.active .q-name {
  color: var(--accent);
}
.q-artist {
  font-size: 11px;
  color: var(--text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 正在播的那首：三个小竖条跳一下，比加个喇叭图标含蓄 */
.q-live {
  display: inline-flex;
  align-items: flex-end;
  gap: 2px;
  height: 12px;
  margin-left: auto;
  flex: none;
}
.q-live i {
  width: 2px;
  border-radius: 2px;
  background: var(--accent);
  animation: q-bounce 900ms ease-in-out infinite;
}
.q-live i:nth-child(1) {
  height: 5px;
  animation-delay: 0ms;
}
.q-live i:nth-child(2) {
  height: 11px;
  animation-delay: 160ms;
}
.q-live i:nth-child(3) {
  height: 7px;
  animation-delay: 320ms;
}
@keyframes q-bounce {
  0%,
  100% {
    transform: scaleY(0.5);
  }
  50% {
    transform: scaleY(1);
  }
}

.q-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: none;
  opacity: 0;
  transition: opacity 140ms var(--ease);
}
.q-row:hover .q-actions,
.q-row.active .q-actions {
  opacity: 1;
}
.q-btn {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 7px;
  color: var(--text-2);
}
.q-btn:hover:not(:disabled) {
  background: var(--surface-2);
  color: var(--text);
}
.q-btn:disabled {
  opacity: 0.3;
}
.up {
  transform: rotate(180deg);
}
.down {
  transform: rotate(0deg);
}

/* 触屏没有 hover，挪动按钮常驻，否则没法排序 */
@media (hover: none) {
  .q-actions {
    opacity: 1;
  }
}

/* 桌面端：右侧抽屉 */
@media (min-width: 861px) {
  .q-mask {
    align-items: stretch;
    justify-content: flex-end;
  }
  .q-panel {
    width: 380px;
    max-height: none;
    border-radius: 18px 0 0 18px;
  }
}

.queue-enter-active,
.queue-leave-active {
  transition: opacity 180ms var(--ease);
}
.queue-enter-active .q-panel,
.queue-leave-active .q-panel {
  transition: transform 220ms var(--ease);
}
.queue-enter-from,
.queue-leave-to {
  opacity: 0;
}
.queue-enter-from .q-panel,
.queue-leave-to .q-panel {
  transform: translateY(16px);
}
@media (min-width: 861px) {
  .queue-enter-from .q-panel,
  .queue-leave-to .q-panel {
    transform: translateX(24px);
  }
}
</style>
