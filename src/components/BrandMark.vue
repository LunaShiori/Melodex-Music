<script setup lang="ts">
withDefaults(defineProps<{ size?: number }>(), { size: 20 })
</script>

<template>
  <svg :width="size ?? 20" :height="size ?? 20" viewBox="0 0 32 32" fill="none" aria-hidden="true">
    <!-- 唱片 + 声波：品牌标识 -->
    <circle cx="16" cy="16" r="13" stroke="currentColor" stroke-width="1.4" opacity="0.35" />
    <circle cx="16" cy="16" r="9" stroke="currentColor" stroke-width="1.4" opacity="0.6" />
    <circle cx="16" cy="16" r="2.6" fill="currentColor" />
    <path d="M4 20c3-2 5-2 8 0s5 2 8 0 5-2 8 0" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" opacity="0.55" />
  </svg>
</template>
