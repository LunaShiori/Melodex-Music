<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'

import Icon from './Icon.vue'
import { usePlayerStore } from '@/stores/player'

const player = usePlayerStore()
const box = ref<HTMLElement | null>(null)
const rows = ref<HTMLElement[]>([])

const title = computed(() => player.current?.title ?? '')
const artists = computed(() => player.current?.artists.join(' / ') ?? '')

/** 当前行滚到可视区中间，手不用动就能跟上 */
async function follow() {
  const index = player.activeLyricIndex
  if (index < 0) return
  await nextTick()
  const row = rows.value[index]
  const wrap = box.value
  if (!row || !wrap) return
  wrap.scrollTo({
    top: Math.max(0, row.offsetTop - wrap.clientHeight / 2 + row.clientHeight / 2),
    behavior: 'smooth',
  })
}

watch(() => player.activeLyricIndex, follow)
watch(
  () => player.lyricOpen,
  (open) => {
    if (open) void follow()
  },
)
</script>

<template>
  <Transition name="fade">
    <aside v-if="player.lyricOpen && player.current" class="lyric">
      <header>
        <div>
          <p class="l-title">{{ title }}</p>
          <p class="l-sub muted">{{ artists }}</p>
        </div>
        <div class="h-actions">
          <button
            class="icon-btn"
            type="button"
            title="全屏歌词"
            @click="player.fullLyric = true"
          >
            <Icon name="external" :size="15" />
          </button>
          <button class="icon-btn" type="button" @click="player.lyricOpen = false">
            <Icon name="close" :size="16" />
          </button>
        </div>
      </header>
      <div ref="box" class="l-body">
        <p v-if="!player.lyricLines.length" class="muted empty">该音源暂未提供歌词</p>
        <p
          v-for="(line, index) in player.lyricLines"
          :key="index"
          :ref="(el) => { if (el) rows[index] = el as HTMLElement }"
          class="l-line"
          :class="{ on: index === player.activeLyricIndex }"
          @click="player.seek(line.timeMs)"
        >
          <span class="l-text">{{ line.text || '♪' }}</span>
          <span v-if="line.translation" class="l-tr">{{ line.translation }}</span>
        </p>
      </div>
    </aside>
  </Transition>
</template>

<style scoped>
.lyric {
  position: fixed;
  top: 0;
  right: 0;
  bottom: var(--player);
  width: 360px;
  display: flex;
  flex-direction: column;
  background: var(--glass);
  backdrop-filter: blur(22px) saturate(1.6);
  border-left: 1px solid var(--line);
  z-index: 15;
  padding: 18px 0 8px;
}
.lyric header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 0 18px 12px;
  border-bottom: 1px solid var(--line);
}
.l-title {
  margin: 0;
  font-size: 14px;
  font-weight: 500;
}
.l-sub {
  margin: 3px 0 0;
  font-size: 11.5px;
}
.h-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}
.l-body {
  flex: 1;
  overflow-y: auto;
  padding: 18px 22px 40px;
  scrollbar-width: thin;
}
.empty {
  font-size: 12.5px;
  text-align: center;
  margin: 40px 0 0;
}
.l-line {
  margin: 0;
  padding: 7px 0;
  font-size: 13.5px;
  line-height: 1.6;
  color: var(--text-3);
  cursor: pointer;
  transition: color 200ms var(--ease), transform 200ms var(--ease);
}
.l-line:hover {
  color: var(--text-2);
}
.l-line.on {
  color: var(--accent);
  font-weight: 500;
  transform: translateX(2px);
}
.l-tr {
  display: block;
  font-size: 11.5px;
  color: var(--text-3);
  margin-top: 2px;
}

/* 手机端的歌词在播放页里，这个抽屉只服务桌面 */
@media (max-width: 860px) {
  .lyric {
    display: none;
  }
}
</style>
