<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useI18n } from '@/lib/i18n'
const { t } = useI18n()
const active = ref(false), maximized = ref(false), full = ref(false)
let win: ReturnType<typeof getCurrentWindow>
let unsubscribe: (() => void) | undefined
async function sync() {
  ;[maximized.value, full.value] = await Promise.all([win.isMaximized(), win.isFullscreen()])
}
onMounted(async () => {
  if (!isTauri() || !/Windows/i.test(navigator.userAgent)) return
  win = getCurrentWindow()
  try {
    await sync()
    unsubscribe = await win.onResized(sync)
    await win.setDecorations(false)
    active.value = true
  } catch { unsubscribe?.() }
})
onUnmounted(() => unsubscribe?.())
</script>
<template><header v-if="active" class="windowbar">
  <div class="drag" data-tauri-drag-region>Melodex Music</div>
  <button :title="t(full?'window.exitFull':'window.full')" @click="win.setFullscreen(!full).then(sync)">⛶</button>
  <button :title="t('window.min')" @click="win.minimize()">−</button>
  <button :disabled="full" :title="t(maximized?'window.restore':'window.max')" @click="win.toggleMaximize().then(sync)">{{maximized?'❐':'□'}}</button>
  <button class="close-window" :title="t('common.close')" @click="win.close()">×</button>
</header></template>
<style scoped>.windowbar{flex:none;display:flex;align-items:stretch;height:36px;background:var(--frost,var(--surface));border-bottom:1px solid var(--line);color:var(--text)}.drag{flex:1;display:flex;align-items:center;padding-left:18px;font-size:12px;font-weight:600;user-select:none}.windowbar button{width:46px;font:18px system-ui;background:transparent;border:0;color:inherit}.windowbar button:hover{background:var(--surface)}.windowbar .close-window:hover{background:#bc253c;color:white}</style>