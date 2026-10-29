<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'

import Icon from './Icon.vue'
import { useMenuStore } from '@/stores/trackMenu'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'

const menu = useMenuStore()
const player = usePlayerStore()
const tasks = useTasksStore()

const track = computed(() => menu.track)

interface Action {
  key: string
  label: string
  icon: string
  run: () => void
}

const actions = computed<Action[]>(() => {
  const target = track.value
  if (!target) return []
  return [
    {
      key: 'play',
      label: '立即播放',
      icon: 'play',
      run: () => void player.playTrack(target),
    },
    {
      key: 'next',
      label: '下一首播放',
      icon: 'next',
      run: () => player.playNext(target),
    },
    {
      key: 'queue',
      label: '加入播放队列',
      icon: 'queue',
      run: () => player.appendToQueue(target),
    },
    {
      key: 'download',
      label: '下载这首',
      icon: 'download',
      run: () => void tasks.enqueue([target]),
    },
    {
      key: 'copy',
      label: '复制「歌名 - 歌手」',
      icon: 'external',
      run: () => {
        const text = `${target.title} - ${target.artists.join(' / ') || '未知歌手'}`
        void navigator.clipboard?.writeText(text)
      },
    },
  ]
})

function run(action: Action) {
  action.run()
  menu.close()
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && menu.open) menu.close()
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  // 滚动和失焦都收起，避免菜单「钉」在旧位置
  window.addEventListener('scroll', () => menu.open && menu.close(), true)
  window.addEventListener('blur', () => menu.open && menu.close())
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <Teleport to="body">
    <Transition name="menu">
      <div v-if="menu.open && track" class="menu-layer" @pointerdown.self="menu.close()">
        <div class="menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }" role="menu">
          <p class="menu-title">
            <span class="menu-name">{{ track.title }}</span>
            <span class="menu-sub">{{ track.artists.join(' / ') || '未知歌手' }}</span>
          </p>
          <button
            v-for="action in actions"
            :key="action.key"
            class="menu-item"
            :class="{ danger: action.key === 'download' }"
            type="button"
            role="menuitem"
            @click="run(action)"
          >
            <Icon :name="action.icon" :size="15" />
            {{ action.label }}
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.menu-layer {
  position: fixed;
  inset: 0;
  z-index: 90;
}

.menu {
  position: absolute;
  width: 208px;
  padding: 6px;
  border-radius: var(--r-lg);
  border: 1px solid var(--line);
  background: linear-gradient(var(--surface), var(--surface)), var(--bg);
  box-shadow: var(--shadow-lg);
  backdrop-filter: blur(18px);
}

.menu-title {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0 0 4px;
  padding: 7px 10px 8px;
  border-bottom: 1px solid var(--line);
}

.menu-name {
  font-size: 12.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu-sub {
  font-size: 11px;
  color: var(--text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 8px 10px;
  border: 0;
  border-radius: var(--r-sm);
  background: transparent;
  color: var(--text-2);
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}

.menu-item:hover {
  background: color-mix(in srgb, var(--text) 7%, transparent);
  color: var(--text);
}

.menu-item.danger:hover {
  color: var(--accent);
}

.menu-enter-active,
.menu-leave-active {
  transition: opacity 100ms var(--ease);
}
.menu-enter-active .menu,
.menu-leave-active .menu {
  transition: transform 100ms var(--ease), opacity 100ms var(--ease);
}
.menu-enter-from,
.menu-leave-to {
  opacity: 0;
}
.menu-enter-from .menu,
.menu-leave-to .menu {
  transform: scale(0.96);
  opacity: 0;
}
</style>
