/**
 * 歌曲右键菜单。只有一个全局实例（挂在 App.vue），任何歌曲行调 `openMenu` 就能弹出来。
 * 状态放 store 而不是组件内部，是为了让 TrackRow / 队列 / 最近播放共用同一份菜单。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'

import type { Track } from '@/lib/types'

export const useMenuStore = defineStore('trackMenu', () => {
  const open = ref(false)
  const x = ref(0)
  const y = ref(0)
  const track = ref<Track | null>(null)

  /** 菜单尺寸的保守估计，用来把菜单夹在视口里，别让它顶出屏幕 */
  const MENU_W = 208
  const MENU_H = 268

  function openMenu(event: MouseEvent, target: Track) {
    const margin = 8
    x.value = Math.min(event.clientX, window.innerWidth - MENU_W - margin)
    y.value = Math.min(event.clientY, window.innerHeight - MENU_H - margin)
    track.value = target
    open.value = true
  }

  function close() {
    open.value = false
    track.value = null
  }

  return { open, x, y, track, openMenu, close }
})
