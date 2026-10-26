<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import Icon from './Icon.vue'
import { useTasksStore } from '@/stores/tasks'

const route = useRoute()
const tasks = useTasksStore()

const items = [
  { name: 'search', path: '/search', label: '搜索', icon: 'search' },
  { name: 'downloads', path: '/downloads', label: '下载', icon: 'download' },
  { name: 'library', path: '/library', label: '曲库', icon: 'library' },
  { name: 'playlists', path: '/playlists', label: '歌单', icon: 'queue' },
  { name: 'settings', path: '/settings', label: '设置', icon: 'settings' },
]

/** 选中项的下标，用来把高亮胶囊滑过去 */
const activeIndex = computed(() => {
  const found = items.findIndex((item) => item.name === route.name)
  return found < 0 ? 0 : found
})
</script>

<template>
  <nav class="tabbar">
    <div class="glass" :style="{ '--count': items.length }">
      <span class="pill" :style="{ '--i': activeIndex }" aria-hidden="true" />
      <RouterLink
        v-for="(item, index) in items"
        :key="item.name"
        :to="item.path"
        class="tab"
        :class="{ active: activeIndex === index }"
      >
        <span class="icon">
          <Icon :name="item.icon" :size="19" />
          <i v-if="item.name === 'downloads' && tasks.counts.active" class="badge mono">
            {{ tasks.counts.active }}
          </i>
        </span>
        <span class="label">{{ item.label }}</span>
      </RouterLink>
    </div>
  </nav>
</template>

<style scoped>
.tabbar {
  display: none;
}

@media (max-width: 860px) {
  .tabbar {
    display: block;
    flex: 0 0 auto;
    padding: 6px 12px calc(env(safe-area-inset-bottom, 0px) + 8px);
    background: transparent;
  }

  /* 液态玻璃：半透明底 + 强模糊 + 顶部高光 + 内外双描边 */
  .glass {
    position: relative;
    display: grid;
    /* 标签页数量由模板里的 --count 传入，增删导航项不用再改样式 */
    grid-template-columns: repeat(var(--count, 4), 1fr);
    padding: 5px;
    border-radius: 24px;
    background: color-mix(in srgb, var(--surface) 52%, transparent);
    backdrop-filter: blur(26px) saturate(1.9);
    border: 1px solid color-mix(in srgb, var(--line-strong) 70%, transparent);
    box-shadow:
      0 12px 34px -14px rgba(0, 0, 0, 0.55),
      inset 0 1px 0 color-mix(in srgb, #fff 22%, transparent);
    overflow: hidden;
  }
  /* 一层顶部的液体反光，模拟玻璃边缘的折射 */
  .glass::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
    background: linear-gradient(
      180deg,
      color-mix(in srgb, #fff 14%, transparent),
      transparent 42%
    );
    opacity: 0.7;
  }

  .pill {
    position: absolute;
    top: 5px;
    bottom: 5px;
    left: 5px;
    width: calc((100% - 10px) / var(--count, 4));
    border-radius: 19px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 26%, transparent);
    transform: translateX(calc(100% * var(--i)));
    transition: transform 380ms var(--ease);
    box-shadow: inset 0 1px 0 color-mix(in srgb, #fff 18%, transparent);
  }

  .tab {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    height: 50px;
    font-size: 10.5px;
    color: var(--text-3);
    border-radius: 19px;
    transition: color 260ms var(--ease);
  }
  .tab.active {
    color: var(--accent);
  }
  .icon {
    position: relative;
    display: grid;
    place-items: center;
    transition: transform 320ms var(--ease);
  }
  .tab.active .icon {
    transform: translateY(-1px) scale(1.08);
  }
  .tab:active .icon {
    transform: scale(0.88);
  }
  .label {
    letter-spacing: 0.02em;
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

@media (prefers-reduced-motion: reduce) {
  .pill,
  .icon {
    transition: none;
  }
}
</style>
