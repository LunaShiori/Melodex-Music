<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import Icon from '@/components/Icon.vue'
import { useOnboardingStore } from '@/stores/onboarding'

const onboarding = useOnboardingStore()
const router = useRouter()

interface Step {
  icon: string
  title: string
  body: string
  bullets: string[]
  action?: { label: string; to: string }
}

const STEPS: Step[] = [
  {
    icon: 'disc',
    title: '欢迎使用 Melodex',
    body: '把网易云、QQ 音乐、酷我拉到同一个界面里搜索、试听、下载。同一首歌在各平台的版本会聚成一行，哪个源有货就从哪个源取。',
    bullets: ['聚合搜索 + 播放 + 下载一体', '登录后按账号权益取最高音质', '失败自动换源，支持断点续传'],
  },
  {
    icon: 'user',
    title: '第一步：登录音源',
    body: '不登录也能用，但平台只肯给 128k / 320k。无损、Hi-Res、臻品母带都要登录后按你的账号权益下发。',
    bullets: [
      '设置 → 账号 → 扫码登录（也可粘贴 Cookie）',
      '登录后才能读到你自己账号下的歌单',
      '凭据只存在本机，不会上传到任何地方',
    ],
    action: { label: '去登录', to: '/settings' },
  },
  {
    icon: 'search',
    title: '第二步：搜索',
    body: '输入歌名或歌手，三家同时返回。同一首歌的多来源会合并成一行，右侧那排彩色小标签就是各家入口。',
    bullets: [
      '⌘ / Ctrl + K 随时聚焦搜索框',
      '点某个来源标签，就下那一家的版本',
      '勾选多首可以批量下载',
    ],
  },
  {
    icon: 'download',
    title: '第三步：下载',
    body: '默认按你设的音质去要；这一档拿不到时会顺着音质阶梯自动降一档。想宁缺毋滥，可以在设置里关掉自动降级。',
    bullets: [
      '下载中心能看到进度和失败原因',
      '下完可以直接「打开文件位置」',
      '曲库支持列表 / 大封面两种浏览方式',
    ],
  },
  {
    icon: 'music',
    title: '几个小技巧',
    body: '到这里就可以开始了。想再看一遍这份引导，去「设置 → 调试模式」点重放即可。',
    bullets: [
      '空格键 播放 / 暂停',
      '点底部播放条封面，进入沉浸式播放页',
      '曲库可以按大小、时长、标题排序',
    ],
  },
]

const index = computed(() => Math.min(onboarding.step, STEPS.length - 1))
const step = computed(() => STEPS[index.value])
const isLast = computed(() => index.value === STEPS.length - 1)

function next() {
  if (isLast.value) {
    onboarding.close()
    return
  }
  onboarding.step = index.value + 1
}

function prev() {
  onboarding.step = Math.max(0, index.value - 1)
}

function go(path: string) {
  onboarding.close()
  void router.push(path)
}
</script>

<template>
  <Transition name="onb">
    <div v-if="onboarding.visible" class="mask" @click.self="onboarding.close()">
      <div class="sheet" role="dialog" aria-modal="true" :aria-label="step.title">
        <button class="close" type="button" title="跳过" @click="onboarding.close()">
          <Icon name="close" :size="16" />
        </button>

        <div class="art">
          <span class="halo" />
          <Icon :name="step.icon" :size="30" />
        </div>

        <p class="counter mono">{{ index + 1 }} / {{ STEPS.length }}</p>
        <h2>{{ step.title }}</h2>
        <p class="body">{{ step.body }}</p>

        <ul class="points">
          <li v-for="point in step.bullets" :key="point">
            <Icon name="check" :size="13" />
            <span>{{ point }}</span>
          </li>
        </ul>

        <footer class="foot">
          <div class="dots" aria-hidden="true">
            <i v-for="(_, i) in STEPS" :key="i" :class="{ on: i === index }" />
          </div>
          <div class="actions">
            <button v-if="step.action" class="btn btn-ghost" type="button" @click="go(step.action.to)">
              {{ step.action.label }}
            </button>
            <button v-if="index > 0" class="btn btn-ghost" type="button" @click="prev">
              上一步
            </button>
            <button class="btn btn-primary" type="button" @click="next">
              {{ isLast ? '开始使用' : '下一步' }}
            </button>
          </div>
        </footer>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.mask {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: grid;
  place-items: center;
  padding: 24px;
  background: color-mix(in srgb, #000 42%, transparent);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
}

.sheet {
  position: relative;
  width: min(460px, 100%);
  max-height: 100%;
  overflow-y: auto;
  padding: 30px 30px 24px;
  border-radius: var(--r-xl);
  background: var(--surface);
  border: 1px solid var(--line-strong);
  box-shadow: var(--shadow-lg);
}

.close {
  position: absolute;
  top: 14px;
  right: 14px;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: var(--r-full);
  color: var(--text-3);
  transition: background var(--dur) var(--ease), color var(--dur) var(--ease);
}
.close:hover {
  background: var(--hover);
  color: var(--text);
}

/* 图标 + 一圈强调色光晕，弱化「说明书」的感觉 */
.art {
  position: relative;
  display: grid;
  place-items: center;
  width: 62px;
  height: 62px;
  margin-bottom: 18px;
  border-radius: 20px;
  color: var(--accent-ink);
  background: linear-gradient(150deg, var(--accent), var(--accent-soft));
  box-shadow: 0 14px 34px -16px var(--accent-glow);
}
.art .halo {
  position: absolute;
  inset: -18px;
  border-radius: 34px;
  background: radial-gradient(closest-side, var(--accent-glow), transparent 78%);
  pointer-events: none;
}
.art :deep(svg) {
  position: relative;
}

.counter {
  margin: 0 0 4px;
  font-size: 11px;
  letter-spacing: 0.08em;
  color: var(--text-3);
}

h2 {
  margin: 0 0 8px;
  font-size: 20px;
  font-weight: 650;
  letter-spacing: -0.01em;
}

.body {
  margin: 0 0 16px;
  font-size: 13.5px;
  line-height: 1.7;
  color: var(--text-2);
}

.points {
  display: grid;
  gap: 9px;
  margin: 0 0 22px;
  padding: 0;
  list-style: none;
}
.points li {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--text-2);
}
.points :deep(svg) {
  flex: 0 0 auto;
  margin-top: 3px;
  color: var(--accent);
}

.foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 16px;
  border-top: 1px solid var(--line);
}

.dots {
  display: flex;
  gap: 5px;
}
.dots i {
  width: 6px;
  height: 6px;
  border-radius: var(--r-full);
  background: var(--line-strong);
  transition: all var(--dur) var(--ease);
}
.dots i.on {
  width: 16px;
  background: var(--accent);
}

.actions {
  display: flex;
  gap: 8px;
}

.onb-enter-active,
.onb-leave-active {
  transition: opacity 220ms var(--ease);
}
.onb-enter-from,
.onb-leave-to {
  opacity: 0;
}
.onb-enter-active .sheet,
.onb-leave-active .sheet {
  transition: transform 260ms var(--ease), opacity 220ms var(--ease);
}
.onb-enter-from .sheet,
.onb-leave-to .sheet {
  transform: translateY(14px) scale(0.98);
  opacity: 0;
}

/* 手机端：贴底弹出，按钮占满一行更好点 */
@media (max-width: 860px) {
  .mask {
    place-items: end center;
    padding: 0;
  }
  .sheet {
    width: 100%;
    border-radius: var(--r-xl) var(--r-xl) 0 0;
    padding: 26px 20px calc(env(safe-area-inset-bottom, 0px) + 18px);
  }
  .foot {
    flex-direction: column-reverse;
    align-items: stretch;
    gap: 14px;
  }
  .dots {
    justify-content: center;
  }
  .actions {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
  }
  .actions .btn {
    height: 42px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .onb-enter-active .sheet,
  .onb-leave-active .sheet {
    transition: none;
  }
}
</style>
