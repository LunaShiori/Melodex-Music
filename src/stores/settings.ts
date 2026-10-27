import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { accentOf } from '@/lib/theme'
import { transport } from '@/lib/transport'
import {
  readDebugMode,
  readUiStyle,
  writeDebugMode,
  writeUiStyle,
  type UiStyleId,
} from '@/lib/ui-style'
import type { Settings } from '@/lib/types'

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings | null>(null)
  const loaded = ref(false)
  /** 界面风格：纯外观偏好，存 localStorage，不进设置文件 */
  const uiStyle = ref<UiStyleId>(readUiStyle())
  /** 调试模式：设置页里手动打开，用于重放引导 / 查看内部状态 */
  const debugMode = ref(readDebugMode())
  const systemDark = ref(
    typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches,
  )

  const resolvedTheme = computed<'light' | 'dark'>(() => {
    const mode = settings.value?.theme ?? 'system'
    if (mode === 'system') return systemDark.value ? 'dark' : 'light'
    return mode
  })

  function applyUiStyle() {
    if (typeof document === 'undefined') return
    document.documentElement.dataset.style = uiStyle.value
  }

  function applyTheme() {
    if (typeof document === 'undefined') return
    document.documentElement.dataset.theme = resolvedTheme.value
    document.documentElement.style.colorScheme = resolvedTheme.value
    // 强调色（主题色）注入 CSS 变量，与明暗主题正交
    const accent = accentOf(settings.value?.accent ?? 'violet')
    const root = document.documentElement
    root.style.setProperty('--accent', accent.accent)
    root.style.setProperty('--accent-soft', accent.soft)
    root.style.setProperty('--accent-ink', accent.ink)
    applyUiStyle()
  }

  function setUiStyle(next: UiStyleId) {
    uiStyle.value = next
    writeUiStyle(next)
    applyUiStyle()
  }

  function setDebugMode(on: boolean) {
    debugMode.value = on
    writeDebugMode(on)
  }

  async function load() {
    settings.value = await transport.getSettings()
    loaded.value = true
    applyTheme()
  }

  async function save(next: Settings) {
    settings.value = next
    applyTheme()
    await transport.saveSettings(next)
  }

  function toggleTheme() {
    if (!settings.value) return
    void save({ ...settings.value, theme: resolvedTheme.value === 'dark' ? 'light' : 'dark' })
  }

  if (typeof window !== 'undefined') {
    window.addEventListener('resize', () => {
      /* 预留：窄屏布局由 CSS 处理 */
    })
    window
      .matchMedia?.('(prefers-color-scheme: dark)')
      .addEventListener?.('change', (event) => {
        systemDark.value = event.matches
        if ((settings.value?.theme ?? 'system') === 'system') applyTheme()
      })
  }

  return {
    settings,
    loaded,
    resolvedTheme,
    uiStyle,
    debugMode,
    load,
    save,
    toggleTheme,
    applyTheme,
    setUiStyle,
    setDebugMode,
  }
})
