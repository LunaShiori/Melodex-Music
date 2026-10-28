<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { RouterView } from 'vue-router'

import PlayerBar from '@/components/PlayerBar.vue'
import QueuePanel from '@/components/QueuePanel.vue'
import LyricView from '@/components/LyricView.vue'
import OnboardingDialog from '@/components/OnboardingDialog.vue'
import SideBar from '@/components/SideBar.vue'
import TabBar from '@/components/TabBar.vue'
import ToastStack from '@/components/ToastStack.vue'
import UpdateNotice from '@/components/UpdateNotice.vue'
import { loadHistory } from '@/lib/history'
import { useOnboardingStore } from '@/stores/onboarding'
import { useSettingsStore } from '@/stores/settings'
import { useTasksStore } from '@/stores/tasks'
import { useUpdateStore } from '@/stores/update'

const settings = useSettingsStore()
const tasks = useTasksStore()
const update = useUpdateStore()
const onboarding = useOnboardingStore()

/**
 * 移动端软键盘弹出时，可视高度会明显小于窗口高度。
 * 这时底部导航和播放条正好被键盘压住（点了也没用），
 * 干脆让位给输入框——用一个 html 上的标记让 CSS 去收起它们。
 */
function syncKeyboard(): void {
  const viewport = window.visualViewport
  if (!viewport) return
  const covered = window.innerHeight - viewport.height
  document.documentElement.dataset.kb = covered > 140 ? '1' : '0'
}

onMounted(async () => {
  loadHistory()
  await Promise.all([settings.load(), tasks.init()])
  // 启动后静默检查一次更新：失败不打扰用户，仅在真正有新版本时提示。
  // 演示模式也跑一次——demo 里备了一份写好的更新清单，这样浏览器里也能走查
  // 更新弹窗（Markdown 说明、手动挑安装包）而不用装 Tauri 外壳。
  void update.check({ silent: true })

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
  <div class="shell">
    <div class="workspace">
      <SideBar />
      <main class="content">
        <RouterView v-slot="{ Component }">
          <Transition name="fade" mode="out-in">
            <component :is="Component" />
          </Transition>
        </RouterView>
      </main>
    </div>
    <PlayerBar />
    <QueuePanel />
    <TabBar />
    <LyricView />
    <OnboardingDialog />
    <UpdateNotice />
    <ToastStack />
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
</style>
