<script setup lang="ts">
import { useRoute } from 'vue-router'

import Icon from './Icon.vue'
import { useTasksStore } from '@/stores/tasks'

const route = useRoute()
const tasks = useTasksStore()

const items = [
  { name: 'search', path: '/search', label: '搜索', icon: 'search' },
  { name: 'downloads', path: '/downloads', label: '下载', icon: 'download' },
  { name: 'library', path: '/library', label: '曲库', icon: 'library' },
  { name: 'settings', path: '/settings', label: '设置', icon: 'settings' },
]
</script>

<template>
  <nav class="tabbar">
    <RouterLink
      v-for="item in items"
      :key="item.name"
      :to="item.path"
      class="tab"
      :class="{ active: route.name === item.name }"
    >
      <span class="icon">
        <Icon :name="item.icon" :size="19" />
        <i v-if="item.name === 'downloads' && tasks.counts.active" class="badge mono">
          {{ tasks.counts.active }}
        </i>
      </span>
      <span>{{ item.label }}</span>
    </RouterLink>
  </nav>
</template>

<style scoped>
.tabbar {
  display: none;
}

@media (max-width: 860px) {
  .tabbar {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    flex: 0 0 auto;
    border-top: 1px solid var(--line);
    background: var(--glass);
    backdrop-filter: blur(16px);
    padding-bottom: env(safe-area-inset-bottom, 0px);
  }
  .tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    height: 54px;
    font-size: 10.5px;
    color: var(--text-3);
  }
  .tab.active {
    color: var(--accent);
  }
  .icon {
    position: relative;
    display: grid;
    place-items: center;
  }
  .badge {
    position: absolute;
    top: -4px;
    right: -8px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    border-radius: 7px;
    background: var(--accent);
    color: #fff;
    font-size: 9px;
    line-height: 14px;
    text-align: center;
  }
}
</style>
