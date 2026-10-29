<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import BrandMark from './BrandMark.vue'
import Icon from './Icon.vue'
import { transport } from '@/lib/transport'
import { ALL_SOURCES, SOURCE_META } from '@/lib/types'
import { usePlayerStore } from '@/stores/player'
import { useSearchStore } from '@/stores/search'
import { useSettingsStore } from '@/stores/settings'
import { useTasksStore } from '@/stores/tasks'

const route = useRoute()
const tasks = useTasksStore()
const search = useSearchStore()
const settings = useSettingsStore()
const player = usePlayerStore()

const navs = computed(() => [
  { name: 'search', path: '/search', label: '搜索', icon: 'search', badge: 0 },
  { name: 'recent', path: '/recent', label: '最近', icon: 'history', badge: 0 },
  { name: 'downloads', path: '/downloads', label: '下载', icon: 'download', badge: tasks.counts.active },
  { name: 'library', path: '/library', label: '曲库', icon: 'library', badge: tasks.completed.length },
  { name: 'playlists', path: '/playlists', label: '歌单', icon: 'queue', badge: 0 },
  { name: 'settings', path: '/settings', label: '设置', icon: 'settings', badge: 0 },
])

const activeSources = computed(() => settings.settings?.sources ?? ALL_SOURCES)
</script>

<template>
  <aside class="sidebar">
    <header class="brand">
      <span class="mark" :class="{ spinning: player.playing }"><BrandMark :size="26" /></span>
      <span class="brand-text">
        <strong>Melodex<i class="music-tag">MUSIC</i></strong>
        <small>多源音乐聚合 · 播放与歌词</small>
      </span>
    </header>

    <nav class="nav">
      <RouterLink
        v-for="item in navs"
        :key="item.name"
        :to="item.path"
        class="nav-item"
        :class="{ active: route.name === item.name }"
      >
        <Icon :name="item.icon" :size="17" />
        <span class="nav-label">{{ item.label }}</span>
        <span v-if="item.badge" class="nav-badge mono">{{ item.badge }}</span>
      </RouterLink>
    </nav>

    <div class="divider" />

    <section class="sources">
      <p class="section-title">音源</p>
      <div
        v-for="source in ALL_SOURCES"
        :key="source"
        class="source-row"
        :class="{ off: !activeSources.includes(source) }"
      >
        <i class="dot" :style="{ background: SOURCE_META[source].color }" />
        <span>{{ SOURCE_META[source].label }}</span>
      </div>
    </section>

    <div class="spacer" />

    <footer class="side-footer">
      <button class="theme-toggle" type="button" @click="settings.toggleTheme()">
        <Icon :name="settings.resolvedTheme === 'dark' ? 'sun' : 'moon'" :size="16" />
        <span>{{ settings.resolvedTheme === 'dark' ? '浅色模式' : '深色模式' }}</span>
      </button>
      <p class="mode">
        <span class="mode-dot" :class="transport.mode" />
        {{ transport.mode === 'tauri' ? '桌面端 · 本地引擎' : '演示模式 · 内置数据' }}
      </p>
      <button v-if="search.searched" class="clear" type="button" @click="search.reset()">
        清空搜索状态
      </button>
    </footer>
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--sidebar);
  flex: 0 0 var(--sidebar);
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 20px 14px 14px;
  border-right: 1px solid var(--line);
  background: var(--surface);
  min-height: 0;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 2px 6px 10px;
}

.mark {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  border-radius: 12px;
  color: #fff;
  background: linear-gradient(148deg, var(--accent) 10%, #22c1c3 120%);
  box-shadow: 0 6px 18px -8px color-mix(in srgb, var(--accent) 70%, transparent);
}
.mark.spinning :deep(svg) {
  animation: spin 8s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.brand-text {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
}
.brand-text strong {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 15px;
  font-weight: 600;
  letter-spacing: 0.01em;
}

/* 「MUSIC」用小方框跟 Melodex 分开：既保留原名，又让名字里带 music 更显眼 */
.music-tag {
  font-style: normal;
  font-size: 8.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  padding: 2px 4px;
  border-radius: 4px;
  color: color-mix(in srgb, var(--accent) 92%, #fff);
  border: 1px solid color-mix(in srgb, var(--accent) 52%, transparent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  transform: translateY(0.5px);
}
.brand-text small {
  font-size: 11px;
  color: var(--text-3);
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 38px;
  padding: 0 10px;
  border-radius: var(--r-sm);
  color: var(--text-2);
  font-size: 13.5px;
  transition: background var(--dur) var(--ease), color var(--dur) var(--ease);
}
.nav-item:hover {
  background: var(--hover);
  color: var(--text);
}
.nav-item.active {
  background: var(--active);
  color: var(--accent);
  font-weight: 500;
}
.nav-label {
  flex: 1;
}
.nav-badge {
  font-size: 10.5px;
  padding: 1px 6px;
  border-radius: var(--r-full);
  background: var(--surface-3);
  color: var(--text-2);
}
.nav-item.active .nav-badge {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  color: var(--accent);
}

.divider {
  height: 1px;
  background: var(--line);
  margin: 0 6px;
}

.sources {
  padding: 0 8px;
}
.sources .section-title {
  margin: 0 0 8px;
  font-size: 11px;
  letter-spacing: 0.08em;
}
.source-row {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 26px;
  font-size: 12.5px;
  color: var(--text-2);
}
.source-row .dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex: 0 0 auto;
}
.source-row.off {
  opacity: 0.42;
  text-decoration: line-through;
  text-decoration-color: var(--text-3);
}

.spacer {
  flex: 1;
}

.side-footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 8px 4px;
  border-top: 1px solid var(--line);
}

.theme-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 8px;
  border-radius: var(--r-sm);
  font-size: 12.5px;
  color: var(--text-2);
}
.theme-toggle:hover {
  background: var(--hover);
  color: var(--text);
}

.mode {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  padding: 0 8px;
  font-size: 11px;
  color: var(--text-3);
}
.mode-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--text-3);
}
.mode-dot.tauri {
  background: var(--accent-2);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 22%, transparent);
}

.clear {
  align-self: flex-start;
  margin: 0 8px;
  font-size: 11px;
  color: var(--text-3);
  text-decoration: underline;
  text-underline-offset: 2px;
}
.clear:hover {
  color: var(--text-2);
}

@media (max-width: 860px) {
  .sidebar {
    display: none;
  }
}
</style>
