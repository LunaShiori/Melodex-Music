<script setup lang="ts">
withDefaults(defineProps<{ size?: number }>(), { size: 20 })

/**
 * 品牌标识：三根圆头竖条组成的波形。
 * 与应用图标同构（图标是它的加色版），侧栏、引导页、桌面/安卓图标是一套。
 */
const BARS = [
  { x: 8.6, y: 10.55, h: 10.9 },
  { x: 14.2, y: 7.5, h: 17 },
  { x: 19.8, y: 10.55, h: 10.9 },
]
</script>

<template>
  <svg :width="size ?? 20" :height="size ?? 20" viewBox="0 0 32 32" fill="none" aria-hidden="true">
    <rect
      v-for="(bar, index) in BARS"
      :key="index"
      :x="bar.x"
      :y="bar.y"
      width="3.4"
      :height="bar.h"
      rx="1.7"
      fill="currentColor"
      :opacity="index === 1 ? 1 : 0.82"
    />
  </svg>
</template>
