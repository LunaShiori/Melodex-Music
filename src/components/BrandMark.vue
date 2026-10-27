<script setup lang="ts">
withDefaults(defineProps<{ size?: number }>(), { size: 20 })

/**
 * 品牌标识：玻璃盘 + 发光均衡器。
 * 与应用图标同构（图标是它的加光加色版），这样侧栏、引导页、桌面图标看起来是一套。
 */
const BARS = [
  { x: 5.2, y: 12.6, h: 6.8 },
  { x: 9.9, y: 10.6, h: 10.8 },
  { x: 14.6, y: 8.6, h: 14.8 },
  { x: 19.3, y: 10.6, h: 10.8 },
  { x: 24, y: 12.6, h: 6.8 },
]
</script>

<template>
  <svg :width="size ?? 20" :height="size ?? 20" viewBox="0 0 32 32" fill="none" aria-hidden="true">
    <circle cx="16" cy="16" r="13.2" stroke="currentColor" stroke-width="1.3" opacity="0.32" />
    <rect
      v-for="(bar, index) in BARS"
      :key="index"
      :x="bar.x"
      :y="bar.y"
      width="2.8"
      :height="bar.h"
      rx="1.4"
      fill="currentColor"
      :opacity="index === 2 ? 1 : 0.78"
    />
  </svg>
</template>
