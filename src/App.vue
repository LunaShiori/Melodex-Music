<script setup lang="ts">
import WindowBar from '@/components/WindowBar.vue'
import VersionSheet from '@/components/VersionSheet.vue'
import { ref, onMounted, onUnmounted } from 'vue'
import { RouterView } from 'vue-router'

import DuplicateReview from '@/components/DuplicateReview.vue'
import PlayerBar from '@/components/PlayerBar.vue'
import QueuePanel from '@/components/QueuePanel.vue'
import TrackMenu from '@/components/TrackMenu.vue'
import LyricView from '@/components/LyricView.vue'
import OnboardingDialog from '@/components/OnboardingDialog.vue'
import SideBar from '@/components/SideBar.vue'
import TabBar from '@/components/TabBar.vue'
import ToastStack from '@/components/ToastStack.vue'
import UpdateNotice from '@/components/UpdateNotice.vue'
import { loadHistory } from '@/lib/history'
import { transport } from '@/lib/transport'
import { useOnboardingStore } from '@/stores/onboarding'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'
import { useTasksStore } from '@/stores/tasks'
import { useUpdateStore } from '@/stores/update'

const settings = useSettingsStore()
const tasks = useTasksStore()
const update = useUpdateStore()
const onboarding = useOnboardingStore()
const player = usePlayerStore()

/**
 * 移动端软键盘弹出时，可视高度会明显小于窗口高度。
 * 这时底部导航和播放条正好被键盘压住（点了也没用），
 * 干脆让位给输入框——用一个 html 上的标记让 CSS 去收起它们。
 */
function syncKeyboard(): void {
  const viewport = window.visualViewport
  if (!viewport) return
  const covered = window.innerHeight - viewport.height - viewport.offsetTop
  document.documentElement.style.setProperty('--visible-height', viewport.height + 'px')
  document.documentElement.dataset.kb = covered > 140 ? '1' : '0'
}

const dock = ref<HTMLElement | null>(null)
const dockHeight = ref(0)
let dockObserver: ResizeObserver | undefined
onMounted(() => {
  if (!dock.value) return
  dockObserver = new ResizeObserver(() => {
    dockHeight.value = dock.value?.offsetHeight ?? 0
  })
  dockObserver.observe(dock.value)
})
onUnmounted(() => dockObserver?.disconnect())

onMounted(async () => {
  loadHistory()
  await Promise.all([settings.load(), tasks.init()])
  // 启动后静默检查一次更新：失败不打扰用户，仅在真正有新版本时提示。
  // 演示模式也跑一次——demo 里备了一份写好的更新清单，这样浏览器里也能走查
  // 更新弹窗（Markdown 说明、手动挑安装包）而不用装 Tauri 外壳。
  void update.check({ silent: true })

  // 「用 Melodex 打开」：双击音频文件时系统把路径作为参数传进来，直接开播
  void transport.startupAudioFiles().then((paths) => {
    if (paths.length) player.playLocalFile(paths[0])
  })

  syncKeyboard()
  window.visualViewport?.addEventListener('resize', syncKeyboard)
  window.visualViewport?.addEventListener('scroll', syncKeyboard)

  // 首装引导：等首屏渲染完再弹，别和启动加载抢帧（只看一次，之后可在设置里重放）
  window.setTimeout(() => onboarding.maybeShowFirstRun(), 700)
})

onUnmounted(() => {
  window.visualViewport?.removeEventListener('resize', syncKeyboard)
  window.visualViewport?.removeEventListener('scroll', syncKeyboard)
})
</script>

<template>
  <div class="shell" :style="{ '--dock-height': dockHeight + 'px' }">
    <WindowBar />
    <UpdateNotice />
    <VersionSheet />
    <div class="workspace">
      <SideBar />
      <main class="content">
        <ToastStack />
        <RouterView v-slot="{ Component }">
          <Transition name="page-shift" mode="out-in">
            <component :is="Component" :key="$route.path" />
          </Transition>
        </RouterView>
      </main>
    </div>
    <div ref="dock" class="playback-dock">
      <Transition name="mini-player"><PlayerBar v-if="player.current" /></Transition>
      <TabBar />
    </div>
    <QueuePanel />
    <TrackMenu />
    <LyricView />
    <OnboardingDialog />
    <DuplicateReview />
  </div>
</template>

<style scoped>
.shell {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  height: 100dvh;
  overflow: hidden;
}

.workspace {
  flex: 1;
  display: flex;
  min-height: 0;
}

.content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  overflow-x: hidden;
  position: relative;
}

@media (max-width: 860px) {
  .content {
    padding-bottom: 8px;
  }
}
.playback-dock { display: contents }
@media (max-width:860px) {
  .shell .playback-dock {
    position:absolute;
    inset:auto 0 0;
    z-index:30;
    display:flex;
    flex-direction:column;
    pointer-events:none;
  }
  .playback-dock :deep(.glass), .playback-dock :deep(.player) { pointer-events:auto }
  .shell .workspace .content {
    padding-bottom:calc(var(--dock-height) + 8px);
    scroll-padding-bottom:calc(var(--dock-height) + 8px);
  }
}
</style>
