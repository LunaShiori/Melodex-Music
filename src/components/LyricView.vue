<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

import Icon from './Icon.vue'
import { coverOf } from '@/lib/artwork'
import { onCoverError } from '@/lib/cover'
import { formatDuration } from '@/lib/format'
import { formatOffset, LYRIC_MODES, LYRIC_THEMES } from '@/lib/lyric-style'
import { SOURCE_META } from '@/lib/types'
import { useLyricStore } from '@/stores/lyric'
import { usePlayerStore } from '@/stores/player'

/**
 * 唯一的歌词页。
 *
 * 以前这里拆成了两个组件：手机的全屏播放页（封面 + 歌词）和全屏歌词页，
 * 同一个界面上两个按钮通向两个长得几乎一样的页面，点返回还会「换一个页面」，
 * 非常割裂。现在只剩这一个：
 * - 手机：点播放条进入，右上角按钮在「纯歌词 / 歌词 + 封面」之间切
 * - 桌面：点左下角封面进入，同一个组件、同一套主题
 */
const player = usePlayerStore()
const lyric = useLyricStore()

const box = ref<HTMLElement | null>(null)
const rows = ref<HTMLElement[]>([])
const scrubbing = ref(false)
const preview = ref<number | null>(null)
const panelOpen = ref(false)

const track = computed(() => player.current)
const cover = computed(() => (track.value ? coverOf(track.value.cover, track.value.fingerprint) : ''))
const sourceMeta = computed(() => (player.variant ? SOURCE_META[player.variant.source] : null))
const hasLyric = computed(() => player.lyricLines.length > 0)
const showCover = computed(() => lyric.mode === 'cover')

const shownPosition = computed(() => preview.value ?? player.position)
const shownProgress = computed(() =>
  player.duration ? Math.min(1, Math.max(0, shownPosition.value / player.duration)) : 0,
)

/** 正在唱的这一行滚到视口中央偏上，下一句提前进入视线 */
async function follow() {
  const index = player.activeLyricIndex
  if (index < 0) return
  await nextTick()
  const row = rows.value[index]
  const wrap = box.value
  if (!row || !wrap) return
  const target = row.offsetTop - wrap.clientHeight * 0.42
  wrap.scrollTo({ top: Math.max(0, target), behavior: 'smooth' })
}

watch(() => player.activeLyricIndex, follow)
watch(
  () => player.detailOpen,
  (open) => {
    if (open) {
      panelOpen.value = false
      void follow()
    }
  },
)
// 切到「纯歌词」版心变了，得重新对一次位置
watch(() => lyric.mode, () => void follow())

function close() {
  if (panelOpen.value) {
    panelOpen.value = false
    return
  }
  player.detailOpen = false
}

function onKeydown(event: KeyboardEvent) {
  if (!player.detailOpen) return
  if (event.key === 'Escape') close()
  if (event.key === ' ') {
    event.preventDefault()
    player.toggle()
  }
}
onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))

/* 进度条拖拽 */
function startScrub(event: PointerEvent) {
  const bar = event.currentTarget as HTMLElement
  const rect = bar.getBoundingClientRect()
  const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
  preview.value = ratio * player.duration
  const move = (e: PointerEvent) => {
    const r = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width))
    preview.value = r * player.duration
  }
  const up = () => {
    if (preview.value !== null) player.seek(preview.value)
    preview.value = null
    scrubbing.value = false
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
  }
  scrubbing.value = true
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
}
</script>

<template>
  <Transition name="lv-sheet">
    <section
      v-if="player.detailOpen"
      class="lv"
      :data-theme="lyric.theme"
      :data-mode="lyric.mode"
      :style="{ '--lv-size': `${lyric.fontSize}px` }"
    >
      <!-- 背景：封面放大成色块，主题决定压在上面的那层怎么盖 -->
      <div class="lv-bg" aria-hidden="true">
        <div class="bg-cover" :style="cover ? { backgroundImage: `url(${cover})` } : undefined" />
        <div class="bg-veil" />
        <div class="bg-blob one" />
        <div class="bg-blob two" />
      </div>

      <header class="lv-head">
        <button class="round" type="button" aria-label="收起" @click="close">
          <Icon name="chevron" :size="20" />
        </button>

        <div class="lv-meta">
          <p class="lv-title" :title="track?.title ?? ''">{{ track?.title ?? '未在播放' }}</p>
          <p class="lv-sub">
            {{ track?.artists.join(' / ') || '未知歌手' }}
            <span v-if="sourceMeta" class="lv-src" :style="{ color: sourceMeta.color }">
              · {{ sourceMeta.short }}
            </span>
          </p>
        </div>

        <div class="lv-tools">
          <button
            class="round"
            type="button"
            :aria-label="showCover ? '切换为纯歌词' : '切换为歌词加封面'"
            :title="showCover ? '切换为纯歌词' : '切换为歌词加封面'"
            @click="lyric.toggleMode()"
          >
            <Icon :name="showCover ? 'lyric' : 'disc'" :size="18" />
          </button>
          <button
            class="round"
            type="button"
            aria-label="歌词主题"
            title="歌词主题"
            :class="{ on: panelOpen }"
            @click="panelOpen = !panelOpen"
          >
            <Icon name="grid" :size="17" />
          </button>
        </div>
      </header>

      <div class="lv-body">
        <div v-if="showCover" class="lv-art" :class="{ disc: lyric.theme === 'vinyl' }">
          <img
            v-if="cover"
            :src="cover"
            :alt="track?.title ?? ''"
            :class="{ spun: lyric.theme === 'vinyl' && player.playing }"
            @error="onCoverError($event, track?.fingerprint ?? 'melodex')"
          />
          <span v-else class="art-fallback"><Icon name="disc" :size="40" /></span>
          <span v-if="lyric.theme === 'vinyl'" class="disc-hole" />
          <span v-if="player.loading" class="art-loading"><span class="spin" /></span>
        </div>

        <div ref="box" class="lv-scroll" :class="{ solo: !showCover }">
          <p v-if="!hasLyric" class="no-lyric">
            <Icon name="lyric" :size="20" />
            这个音源没有提供歌词
          </p>
          <p
            v-for="(line, index) in player.lyricLines"
            :key="index"
            :ref="(el) => { if (el) rows[index] = el as HTMLElement }"
            class="line"
            :class="{ on: index === player.activeLyricIndex }"
            @click="player.seek(line.timeMs)"
          >
            <span class="main">{{ line.text || '♪' }}</span>
            <span v-if="line.translation" class="tr">{{ line.translation }}</span>
          </p>
        </div>
      </div>

      <div v-if="player.error" class="lv-err">
        <Icon name="alert" :size="15" />
        <div class="err-text">
          <p class="err-label">{{ player.error.label }}</p>
          <p class="err-hint">{{ player.error.hint }}</p>
        </div>
        <button class="btn btn-sm" type="button" @click="player.toggle()">重试</button>
      </div>

      <footer class="lv-foot">
        <!-- 偏移快调：边听边对齐，不用跑一趟设置页 -->
        <div v-if="hasLyric" class="offset">
          <span class="offset-hint">歌词偏移</span>
          <button
            class="offset-btn"
            type="button"
            title="歌词提前 0.1 秒"
            :disabled="!lyric.canNudgeEarlier"
            @click="lyric.nudgeOffset(-1)"
          >
            −
          </button>
          <span class="offset-value mono">{{ formatOffset(lyric.offsetMs) }}</span>
          <button
            class="offset-btn"
            type="button"
            title="歌词延后 0.1 秒"
            :disabled="!lyric.canNudgeLater"
            @click="lyric.nudgeOffset(1)"
          >
            +
          </button>
          <button v-if="lyric.offsetMs" class="offset-reset" type="button" @click="lyric.setOffset(0)">
            归零
          </button>
        </div>

        <div class="timeline">
          <span class="t mono">{{ formatDuration(shownPosition) }}</span>
          <div class="bar" :class="{ scrubbing }" @pointerdown.prevent="startScrub">
            <div class="rail" />
            <div class="fill" :style="{ width: `${shownProgress * 100}%` }" />
            <div class="knob" :style="{ left: `${shownProgress * 100}%` }" />
          </div>
          <span class="t mono">{{ formatDuration(player.duration) }}</span>
        </div>

        <div class="controls">
          <button class="ctrl" type="button" :disabled="!track" @click="player.prev()">
            <Icon name="prev" :size="24" />
          </button>
          <button class="big" type="button" :disabled="!track" @click="player.toggle()">
            <span v-if="player.loading" class="spin dark" />
            <Icon
              v-else
              :name="player.playing ? 'pause' : 'play'"
              :size="25"
              :filled="!player.playing"
            />
          </button>
          <button class="ctrl" type="button" :disabled="!track" @click="player.next()">
            <Icon name="next" :size="24" />
          </button>
        </div>
      </footer>

      <!-- 主题抽屉：模式 + 主题。字号统一在设置页调，这里不放 -->
      <Transition name="panel">
        <div v-if="panelOpen" class="lv-panel" @click.self="panelOpen = false">
          <div class="panel-card">
            <header>
              <p class="panel-title">歌词显示</p>
              <button class="icon-btn" type="button" @click="panelOpen = false">
                <Icon name="close" :size="16" />
              </button>
            </header>

            <p class="panel-label">模式</p>
            <div class="modes">
              <button
                v-for="item in LYRIC_MODES"
                :key="item.id"
                class="mode"
                type="button"
                :class="{ on: lyric.mode === item.id }"
                @click="lyric.setMode(item.id)"
              >
                <Icon :name="item.icon" :size="16" />
                <span class="mode-text">
                  <strong>{{ item.label }}</strong>
                  <small>{{ item.desc }}</small>
                </span>
              </button>
            </div>

            <p class="panel-label">主题</p>
            <div class="themes">
              <button
                v-for="item in LYRIC_THEMES"
                :key="item.id"
                class="theme"
                type="button"
                :class="{ on: lyric.theme === item.id }"
                @click="lyric.setTheme(item.id)"
              >
                <span
                  class="theme-swatch"
                  :style="{ '--sw-a': item.preview[0], '--sw-b': item.preview[1] }"
                >
                  <Icon v-if="lyric.theme === item.id" name="check" :size="13" />
                </span>
                <span class="theme-name">{{ item.label }}</span>
              </button>
            </div>

            <p class="panel-tip">字号在「设置 → 歌词」里调，手机端同样生效</p>
          </div>
        </div>
      </Transition>
    </section>
  </Transition>
</template>

<style scoped>
/* ========== 骨架 ========== */
.lv {
  position: fixed;
  inset: 0;
  z-index: 62;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  color: var(--lv-fg);
  font-family: var(--lv-font, var(--font-sans));
}

.lv-bg {
  position: absolute;
  inset: 0;
  overflow: hidden;
}
.bg-cover {
  position: absolute;
  inset: -30%;
  background-size: cover;
  background-position: center;
  filter: blur(var(--lv-blur, 70px)) saturate(var(--lv-sat, 165%));
  opacity: var(--lv-cover-opacity, 0.55);
  transform: scale(1.12);
  transition: background-image 420ms var(--ease);
}
.bg-veil {
  position: absolute;
  inset: 0;
  background: var(--lv-veil);
  backdrop-filter: blur(var(--lv-veil-blur, 24px)) saturate(130%);
}
.bg-blob {
  position: absolute;
  width: 62vmax;
  height: 62vmax;
  border-radius: 50%;
  filter: blur(90px);
  opacity: var(--lv-blob-opacity, 0);
  pointer-events: none;
}
.bg-blob.one {
  top: -28vmax;
  left: -18vmax;
  background: radial-gradient(circle, var(--lv-blob-a, transparent), transparent 66%);
}
.bg-blob.two {
  right: -22vmax;
  bottom: -30vmax;
  background: radial-gradient(circle, var(--lv-blob-b, transparent), transparent 66%);
}

/* ========== 主题：极光 ========== */
.lv[data-theme='aurora'] {
  --lv-fg: #fff;
  --lv-dim: rgba(255, 255, 255, 0.32);
  --lv-dim-2: rgba(255, 255, 255, 0.5);
  --lv-accent: var(--accent);
  --lv-panel-bg: rgba(20, 20, 28, 0.72);
  --lv-panel-line: rgba(255, 255, 255, 0.12);
  --lv-veil: radial-gradient(120% 70% at 50% 0%, transparent 0%, rgba(6, 6, 12, 0.94) 78%),
    rgba(8, 8, 14, 0.74);
  --lv-cover-opacity: 0.55;
  --lv-line-opacity: 0.42;
}
.lv[data-theme='aurora'] .line.on .main {
  color: #fff;
  text-shadow: 0 0 26px color-mix(in srgb, var(--accent) 55%, transparent);
}

/* ========== 主题：玻璃（浅色磨砂） ========== */
.lv[data-theme='glass'] {
  --lv-fg: #1b2036;
  --lv-dim: rgba(27, 32, 54, 0.3);
  --lv-dim-2: rgba(27, 32, 54, 0.46);
  --lv-accent: var(--accent);
  --lv-panel-bg: rgba(255, 255, 255, 0.82);
  --lv-panel-line: rgba(27, 32, 54, 0.1);
  --lv-veil: radial-gradient(120% 70% at 50% 0%, rgba(255, 255, 255, 0.42) 0%, rgba(247, 249, 255, 0.92) 76%),
    rgba(247, 249, 255, 0.86);
  --lv-cover-opacity: 0.34;
  --lv-blob-opacity: 0.5;
  --lv-blob-a: color-mix(in srgb, var(--accent) 66%, transparent);
  --lv-blob-b: color-mix(in srgb, var(--accent-2, #12b8a0) 60%, transparent);
  --lv-blur: 84px;
  --lv-line-opacity: 0.66;
  --lv-dim-2: rgba(27, 32, 54, 0.54);
}
.lv[data-theme='glass'] .line.on .main {
  color: color-mix(in srgb, var(--accent) 72%, #101528);
  text-shadow: 0 1px 0 rgba(255, 255, 255, 0.6);
}

/* ========== 主题：黑胶（暖色） ========== */
.lv[data-theme='vinyl'] {
  --lv-fg: #f4ece0;
  --lv-dim: rgba(244, 236, 224, 0.28);
  --lv-dim-2: rgba(244, 236, 224, 0.46);
  --lv-accent: #e0a94f;
  --lv-panel-bg: rgba(24, 21, 17, 0.86);
  --lv-panel-line: rgba(224, 169, 79, 0.18);
  --lv-veil: radial-gradient(110% 60% at 50% 8%, rgba(60, 44, 22, 0.5) 0%, rgba(6, 6, 8, 0.96) 74%),
    rgba(6, 6, 8, 0.9);
  --lv-cover-opacity: 0.18;
  --lv-blur: 90px;
  --lv-line-opacity: 0.38;
}
.lv[data-theme='vinyl'] .line.on .main {
  color: #ffd89a;
  text-shadow: 0 0 24px rgba(224, 169, 79, 0.45);
}

/* ========== 主题：词卡（纸面衬线） ========== */
.lv[data-theme='ink'] {
  --lv-fg: #241c12;
  --lv-dim: rgba(36, 28, 18, 0.32);
  --lv-dim-2: rgba(36, 28, 18, 0.5);
  --lv-accent: #8a5a2b;
  --lv-panel-bg: rgba(253, 250, 244, 0.94);
  --lv-panel-line: rgba(36, 28, 18, 0.12);
  --lv-veil: radial-gradient(120% 76% at 50% 0%, #fbf7ee 0%, #f1ead9 74%),
    #f5efe2;
  --lv-cover-opacity: 0.12;
  --lv-blur: 96px;
  --lv-sat: 60%;
  --lv-font: 'Songti SC', 'Noto Serif SC', 'Source Han Serif SC', Georgia, serif;
  --lv-line-opacity: 0.7;
  --lv-dim-2: rgba(36, 28, 18, 0.56);
}
.lv[data-theme='ink'] .line.on .main {
  color: #1a1409;
  font-weight: 600;
  letter-spacing: 0.02em;
}

/* 深色模式下把纸面换成墨面，夜里不刺眼 */
:global([data-theme='dark']) .lv[data-theme='ink'] {
  --lv-fg: #f2e9d6;
  --lv-dim: rgba(242, 233, 214, 0.28);
  --lv-dim-2: rgba(242, 233, 214, 0.46);
  --lv-accent: #d8a768;
  --lv-panel-bg: rgba(24, 22, 19, 0.92);
  --lv-panel-line: rgba(242, 233, 214, 0.14);
  --lv-veil: radial-gradient(120% 76% at 50% 0%, #17151a 0%, #0c0b0d 78%), #0d0c0f;
  --lv-cover-opacity: 0.16;
  --lv-line-opacity: 0.42;
}
:global([data-theme='dark']) .lv[data-theme='ink'] .line.on .main {
  color: #ffe6bd;
}
:global([data-theme='dark']) .lv[data-theme='glass'] {
  --lv-fg: #eef1ff;
  --lv-dim: rgba(238, 241, 255, 0.32);
  --lv-dim-2: rgba(238, 241, 255, 0.48);
  --lv-panel-bg: rgba(22, 24, 34, 0.86);
  --lv-panel-line: rgba(238, 241, 255, 0.14);
  --lv-veil: radial-gradient(120% 70% at 50% 0%, rgba(30, 34, 52, 0.6) 0%, rgba(10, 11, 18, 0.94) 78%),
    rgba(10, 11, 18, 0.86);
}
:global([data-theme='dark']) .lv[data-theme='glass'] .line.on .main {
  color: color-mix(in srgb, var(--accent) 46%, #ffffff);
  text-shadow: 0 0 24px color-mix(in srgb, var(--accent) 40%, transparent);
}

/* ========== 顶栏 ========== */
.lv-head,
.lv-body,
.lv-foot {
  position: relative;
  z-index: 1;
}
.lv-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: calc(env(safe-area-inset-top, 0px) + 12px) 18px 8px;
}
.round {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  color: var(--lv-fg);
  background: color-mix(in srgb, var(--lv-fg) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--lv-fg) 16%, transparent);
  backdrop-filter: blur(12px);
  transition: transform 140ms var(--ease), background var(--dur) var(--ease);
}
.round:active {
  transform: scale(0.92);
}
.round.on {
  color: var(--lv-accent);
  border-color: color-mix(in srgb, var(--lv-accent) 56%, transparent);
}
.lv-meta {
  flex: 1;
  min-width: 0;
  text-align: center;
}
.lv-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.lv-sub {
  margin: 3px 0 0;
  font-size: 11.5px;
  color: var(--lv-dim-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.lv-src {
  font-weight: 500;
}
.lv-tools {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* ========== 主体 ========== */
.lv-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 0 18px;
}
.lv-art {
  position: relative;
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  margin: 6px auto 4px;
}
.lv-art img,
.art-fallback {
  width: min(56vw, 30vh, 232px);
  aspect-ratio: 1;
  border-radius: 22px;
  object-fit: cover;
  display: block;
  box-shadow: 0 26px 60px -26px rgba(0, 0, 0, 0.62);
}
.art-fallback {
  display: grid;
  place-items: center;
  background: color-mix(in srgb, var(--lv-fg) 10%, transparent);
  color: var(--lv-dim-2);
}
/* 黑胶主题：圆形唱片 + 转轴，播放时转 */
.lv-art.disc img {
  border-radius: 50%;
  box-shadow: 0 0 0 8px rgba(0, 0, 0, 0.45), 0 26px 60px -20px rgba(0, 0, 0, 0.8);
  animation: spin 22s linear infinite;
  animation-play-state: paused;
}
.lv-art.disc img.spun {
  animation-play-state: running;
}
.disc-hole {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 13%;
  aspect-ratio: 1;
  border-radius: 50%;
  background: #08080a;
  box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.16);
}
.art-loading {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  border-radius: 22px;
  background: rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(2px);
}
.spin {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: #fff;
  animation: spin 0.75s linear infinite;
}
.spin.dark {
  border-color: color-mix(in srgb, var(--lv-fg) 26%, transparent);
  border-top-color: var(--lv-fg);
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* ========== 歌词 ========== */
.lv-scroll {
  position: relative; /* 滚动容器的 offsetParent 必须是它自己，否则 offsetTop 会算到整页上 */
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 10px 6px 26px;
  scrollbar-width: none;
  -webkit-mask-image: linear-gradient(to bottom, transparent, #000 7%, #000 90%, transparent);
  mask-image: linear-gradient(to bottom, transparent, #000 7%, #000 90%, transparent);
}
.lv-scroll::-webkit-scrollbar {
  display: none;
}
/* 纯歌词模式：上下留白把视线收到中间 */
.lv-scroll.solo {
  padding: 24vh 6px 26vh;
}
.no-lyric {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  margin: 12vh 0 0;
  font-size: 13px;
  color: var(--lv-dim-2);
}
.line {
  margin: 0;
  padding: 9px 0;
  text-align: center;
  cursor: pointer;
  opacity: var(--lv-line-opacity, 0.42);
  transition: opacity 260ms var(--ease), transform 260ms var(--ease);
}
.line .main {
  display: block;
  font-size: var(--lv-size, 22px);
  line-height: 1.5;
  font-weight: 500;
  letter-spacing: 0.01em;
  color: var(--lv-dim-2);
  transition: color 260ms var(--ease);
}
.line .tr {
  display: block;
  margin-top: 4px;
  font-size: calc(var(--lv-size, 22px) * 0.6);
  color: var(--lv-dim);
  transition: color 260ms var(--ease);
}
.line.on {
  opacity: 1;
  transform: scale(1.02);
}
.line.on .main {
  color: var(--lv-fg);
  font-weight: 650;
}
.line.on .tr {
  color: var(--lv-dim-2);
}

/* ========== 错误提示 ========== */
.lv-err {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 18px 8px;
  padding: 10px 12px;
  border-radius: var(--r-md);
  background: color-mix(in srgb, var(--danger) 82%, #000 6%);
  color: #fff;
}
.err-text {
  flex: 1;
  min-width: 0;
}
.err-label {
  margin: 0;
  font-size: 12px;
  font-weight: 600;
}
.err-hint {
  margin: 2px 0 0;
  font-size: 11px;
  opacity: 0.86;
  line-height: 1.5;
}

/* ========== 底栏 ========== */
.lv-foot {
  flex: 0 0 auto;
  padding: 4px 20px calc(env(safe-area-inset-bottom, 0px) + 16px);
}
.offset {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  margin-bottom: 2px;
  font-size: 11px;
  color: var(--lv-dim-2);
}
.offset-hint {
  margin-right: 2px;
}
.offset-btn,
.offset-reset {
  display: grid;
  place-items: center;
  height: 22px;
  min-width: 22px;
  padding: 0 6px;
  border-radius: var(--r-full);
  color: var(--lv-fg);
  background: color-mix(in srgb, var(--lv-fg) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--lv-fg) 16%, transparent);
  font-size: 13px;
  line-height: 1;
}
.offset-btn:disabled {
  opacity: 0.32;
}
.offset-value {
  min-width: 46px;
  text-align: center;
  font-size: 11px;
  color: var(--lv-fg);
}
.offset-reset {
  font-size: 10.5px;
  color: var(--lv-dim-2);
}

.timeline {
  display: flex;
  align-items: center;
  gap: 10px;
}
.t {
  width: 42px;
  flex: 0 0 auto;
  font-size: 10.5px;
  color: var(--lv-dim-2);
}
.t:last-child {
  text-align: right;
}
.bar {
  position: relative;
  flex: 1;
  height: 26px;
  display: flex;
  align-items: center;
  touch-action: none;
}
.rail,
.fill {
  position: absolute;
  left: 0;
  height: 5px;
  border-radius: var(--r-full);
}
.rail {
  right: 0;
  background: color-mix(in srgb, var(--lv-fg) 18%, transparent);
}
.fill {
  background: linear-gradient(90deg, var(--lv-accent), color-mix(in srgb, var(--lv-accent) 60%, #fff));
  box-shadow: 0 0 12px -2px color-mix(in srgb, var(--lv-accent) 70%, transparent);
}
.bar .knob {
  position: absolute;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #fff;
  border: 2px solid var(--lv-accent);
  transform: translateX(-50%);
  transition: transform 140ms var(--ease);
}
.bar.scrubbing .knob {
  transform: translateX(-50%) scale(1.25);
}

.controls {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 28px;
  margin-top: 2px;
}
.ctrl {
  display: grid;
  place-items: center;
  width: 46px;
  height: 46px;
  border-radius: 50%;
  color: var(--lv-fg);
  transition: transform 130ms var(--ease), background var(--dur) var(--ease);
}
.ctrl:active {
  transform: scale(0.9);
  background: color-mix(in srgb, var(--lv-fg) 10%, transparent);
}
.ctrl:disabled {
  opacity: 0.35;
}
.big {
  display: grid;
  place-items: center;
  width: 62px;
  height: 62px;
  border-radius: 50%;
  background: var(--lv-accent);
  color: #fff;
  box-shadow: 0 14px 30px -12px color-mix(in srgb, var(--lv-accent) 82%, transparent);
  transition: transform 140ms var(--ease);
}
.big:active {
  transform: scale(0.93);
}
.big:disabled {
  opacity: 0.4;
  box-shadow: none;
}
.lv[data-theme='ink'] .big,
.lv[data-theme='glass'] .big {
  color: #fff;
}

/* ========== 主题抽屉 ========== */
.lv-panel {
  position: absolute;
  inset: 0;
  z-index: 3;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  background: rgba(6, 6, 10, 0.34);
  backdrop-filter: blur(2px);
}
.panel-card {
  width: 100%;
  max-width: 460px;
  padding: 16px 16px calc(env(safe-area-inset-bottom, 0px) + 16px);
  border-radius: 22px 22px 0 0;
  background: var(--lv-panel-bg);
  border: 1px solid var(--lv-panel-line);
  border-bottom: none;
  color: var(--lv-fg);
  backdrop-filter: blur(26px) saturate(150%);
}
.panel-card header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}
.panel-title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.panel-label {
  margin: 12px 0 6px;
  font-size: 11px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--lv-dim);
}
.modes {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
.mode {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 10px;
  border-radius: var(--r-md);
  border: 1px solid var(--lv-panel-line);
  color: var(--lv-fg);
  text-align: left;
  transition: all var(--dur) var(--ease);
}
.mode.on {
  border-color: color-mix(in srgb, var(--lv-accent) 58%, transparent);
  background: color-mix(in srgb, var(--lv-accent) 12%, transparent);
}
.mode-text {
  min-width: 0;
}
.mode-text strong {
  display: block;
  font-size: 12px;
  font-weight: 600;
}
.mode-text small {
  display: block;
  margin-top: 1px;
  font-size: 10px;
  color: var(--lv-dim);
}
.themes {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}
.theme {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  padding: 6px 4px;
  border-radius: var(--r-md);
  border: 1px solid transparent;
  color: var(--lv-dim-2);
  font-size: 11px;
}
.theme.on {
  border-color: var(--lv-panel-line);
  color: var(--lv-fg);
}
.theme-swatch {
  display: grid;
  place-items: center;
  width: 100%;
  aspect-ratio: 1;
  border-radius: 12px;
  background: linear-gradient(150deg, var(--sw-a), var(--sw-b));
  color: #fff;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
}
.panel-tip {
  margin: 12px 0 0;
  font-size: 10.5px;
  color: var(--lv-dim);
  text-align: center;
}

/* ========== 过渡 ========== */
.lv-sheet-enter-active,
.lv-sheet-leave-active {
  transition: transform 360ms var(--ease), opacity 300ms var(--ease);
}
.lv-sheet-enter-from,
.lv-sheet-leave-to {
  transform: translateY(100%);
  opacity: 0.4;
}
.panel-enter-active,
.panel-leave-active {
  transition: opacity 200ms var(--ease);
}
.panel-enter-from,
.panel-leave-to {
  opacity: 0;
}

/* ========== 桌面端：封面在左、歌词在右 ========== */
@media (min-width: 861px) {
  .lv-head {
    padding: calc(env(safe-area-inset-top, 0px) + 18px) 26px 10px;
  }
  .lv-body {
    padding: 0 40px;
    flex-direction: row;
    align-items: center;
    gap: 48px;
    justify-content: center;
  }
  .lv[data-mode='pure'] .lv-body {
    flex-direction: column;
    gap: 0;
  }
  .lv-art {
    margin: 0;
  }
  .lv-art img,
  .art-fallback {
    width: clamp(220px, 24vw, 336px);
    border-radius: 26px;
  }
  .lv-scroll {
    max-width: 640px;
    width: 100%;
    height: 100%;
    padding: 14vh 8px 14vh;
  }
  .lv-scroll.solo {
    max-width: 760px;
    padding: 26vh 8px 26vh;
  }
  .lv[data-mode='cover'] .lv-scroll {
    height: min(70vh, 620px);
    align-self: center;
    flex: 0 1 640px;
  }
  .lv-foot {
    padding: 8px 40px 20px;
  }
  .lv-foot .timeline,
  .lv-foot .controls {
    max-width: 720px;
    margin: 0 auto;
  }
  .lv-foot .controls {
    margin-top: 4px;
  }
  .lv-panel {
    align-items: center;
  }
  .panel-card {
    border-radius: 22px;
    border-bottom: 1px solid var(--lv-panel-line);
    max-width: 420px;
  }
}

@media (max-width: 400px) {
  .line {
    padding: 8px 0;
  }
}
</style>
