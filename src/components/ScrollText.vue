<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

/**
 * 单行文本，放不下才滚动。
 *
 * 播放条上的位置只够一行字，歌词一长就被省略号吃掉——而「当前唱到哪句」
 * 恰恰是用户在播放条上最想看到的信息。做法是：量一次实际宽度，
 * 只有真的溢出才把内容复制一份做首尾相接的匀速跑马灯；放得下就静静待着，
 * 免得短句也跟着抖。
 */
const props = withDefaults(
  defineProps<{
    text: string
    /** 每秒滚多少像素，越慢越省眼 */
    speed?: number
    /** 开头停顿，读完第一屏再动 */
    delay?: number
  }>(),
  { speed: 26, delay: 900 },
)

const outer = ref<HTMLElement | null>(null)
const inner = ref<HTMLElement | null>(null)
const overflowing = ref(false)
const distance = ref(0)

/** 一次滚完要多久：按实际位移算，长句不会比短句跑得快 */
const duration = computed(() => Math.max(4, distance.value / props.speed + props.delay / 1000))

function measure() {
  const box = outer.value
  const text = inner.value
  if (!box || !text) return
  const width = text.scrollWidth
  distance.value = width
  overflowing.value = width > box.clientWidth + 2
}

let observer: ResizeObserver | null = null

onMounted(async () => {
  await nextTick()
  measure()
  if (typeof ResizeObserver !== 'undefined' && outer.value) {
    observer = new ResizeObserver(() => measure())
    observer.observe(outer.value)
  }
})

onBeforeUnmount(() => observer?.disconnect())

watch(
  () => props.text,
  async () => {
    overflowing.value = false
    await nextTick()
    measure()
  },
)
</script>

<template>
  <span ref="outer" class="scroll-text">
    <span
      ref="inner"
      class="track"
      :class="{ moving: overflowing }"
      :style="
        overflowing
          ? { animationDuration: `${duration}s`, animationDelay: `${delay / 1000}s`, '--gap': '42px' }
          : undefined
      "
    >
      <span class="chunk">{{ props.text }}</span>
      <!-- 复制一份接在后面，滚动到一半时正好接上，看不出接缝；对读屏是重复内容，藏掉 -->
      <span v-if="overflowing" class="chunk gap" aria-hidden="true">{{ props.text }}</span>
    </span>
  </span>
</template>

<style scoped>
.scroll-text {
  display: block;
  overflow: hidden;
  white-space: nowrap;
  min-width: 0;
}
.track {
  display: inline-flex;
  align-items: baseline;
  max-width: none;
}
.track.moving {
  animation-name: marquee;
  animation-timing-function: linear;
  animation-iteration-count: infinite;
  animation-fill-mode: both;
}
.chunk {
  display: inline-block;
  padding-right: 0;
}
.chunk.gap {
  padding-left: var(--gap, 42px);
}
/* 首尾相接的循环：位移刚好是一份内容 + 间隔 */
@keyframes marquee {
  from {
    transform: translateX(0);
  }
  to {
    transform: translateX(calc(-50% - var(--gap, 42px) / 2));
  }
}
.scroll-text:hover .track.moving {
  animation-play-state: paused;
}
@media (prefers-reduced-motion: reduce) {
  .track.moving {
    animation: none;
  }
}
</style>
