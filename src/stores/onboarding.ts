import { defineStore } from 'pinia'
import { ref } from 'vue'

const SEEN_KEY = 'melodex.onboarding.seen'

/**
 * 新手引导。
 *
 * 首次安装自动弹一次（只看一次，关闭即记为已看过）；之后可以在
 * 设置 → 调试模式里随时重放，方便检查引导页本身和功能区是否正常。
 */
export const useOnboardingStore = defineStore('onboarding', () => {
  const visible = ref(false)
  const step = ref(0)

  function hasSeen(): boolean {
    try {
      return localStorage.getItem(SEEN_KEY) === '1'
    } catch {
      // 存不进去（隐私模式）时不要每次启动都弹，当作已看过
      return true
    }
  }

  function markSeen() {
    try {
      localStorage.setItem(SEEN_KEY, '1')
    } catch {
      /* 忽略 */
    }
  }

  function open() {
    step.value = 0
    visible.value = true
  }

  function close() {
    visible.value = false
    markSeen()
  }

  /** 首次启动用：没看过才弹，不打扰老用户 */
  function maybeShowFirstRun() {
    if (!hasSeen()) open()
  }

  /** 调试用：清掉标记并立刻重新播一遍 */
  function replay() {
    try {
      localStorage.removeItem(SEEN_KEY)
    } catch {
      /* 忽略 */
    }
    open()
  }

  return { visible, step, open, close, markSeen, hasSeen, maybeShowFirstRun, replay }
})
