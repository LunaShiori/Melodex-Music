<script setup lang="ts">
import { onMounted } from 'vue'
import { RouterView } from 'vue-router'

import PlayerBar from '@/components/PlayerBar.vue'
import LyricPanel from '@/components/LyricPanel.vue'
import NowPlaying from '@/components/NowPlaying.vue'
import SideBar from '@/components/SideBar.vue'
import TabBar from '@/components/TabBar.vue'
import ToastStack from '@/components/ToastStack.vue'
import UpdateNotice from '@/components/UpdateNotice.vue'
import { transport } from '@/lib/transport'
import { useSettingsStore } from '@/stores/settings'
import { useTasksStore } from '@/stores/tasks'
import { useUpdateStore } from '@/stores/update'

const settings = useSettingsStore()
const tasks = useTasksStore()
const update = useUpdateStore()

onMounted(async () => {
  await Promise.all([settings.load(), tasks.init()])
  // 启动后静默检查一次更新：失败不打扰用户，仅在真正有新版本时提示
  if (transport.mode === 'tauri') void update.check({ silent: true })
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
    <LyricPanel />
    <TabBar />
    <NowPlaying />
    <UpdateNotice />
    <ToastStack />
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100dvh;
  overflow: hidden;
  background: var(--bg);
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
