import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import {
  clampFont,
  clampOffset,
  isLyricMode,
  isLyricTheme,
  LYRIC_FONT_DEFAULT,
  LYRIC_OFFSET_STEP,
  LYRIC_OFFSET_LIMIT,
  type LyricMode,
  type LyricThemeId,
} from '@/lib/lyric-style'

/**
 * 歌词显示偏好。
 *
 * 只存 localStorage，不进后端设置文件：这是**设备级**的外观偏好（屏幕大小、
 * 个人视力和耳机延迟各设备都不同），而且老版本读到不认识的字段也不会出事。
 *
 * 字号只在设置页里调（手机端也一样），歌词页里只放「偏移」快调——
 * 调偏移是个边听边试的动作，跑一趟设置页太打断。
 */

const KEY = 'melodex.lyric'

interface Stored {
  fontSize?: unknown
  offsetMs?: unknown
  mode?: unknown
  theme?: unknown
  showInBar?: unknown
}

function read(): {
  fontSize: number
  offsetMs: number
  mode: LyricMode
  theme: LyricThemeId
  showInBar: boolean
} {
  const fallback = {
    fontSize: LYRIC_FONT_DEFAULT,
    offsetMs: 0,
    mode: 'cover' as LyricMode,
    theme: 'aurora' as LyricThemeId,
    showInBar: true,
  }
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return fallback
    const parsed = JSON.parse(raw) as Stored
    return {
      fontSize: clampFont(Number(parsed.fontSize)),
      offsetMs: clampOffset(Number(parsed.offsetMs)),
      mode: isLyricMode(parsed.mode) ? parsed.mode : fallback.mode,
      theme: isLyricTheme(parsed.theme) ? parsed.theme : fallback.theme,
      showInBar: parsed.showInBar !== false,
    }
  } catch {
    // 隐私模式 / 存了半截 JSON：都用默认值，不影响播放
    return fallback
  }
}

export const useLyricStore = defineStore('lyric', () => {
  const initial = read()

  const fontSize = ref(initial.fontSize)
  const offsetMs = ref(initial.offsetMs)
  const mode = ref<LyricMode>(initial.mode)
  const theme = ref<LyricThemeId>(initial.theme)
  const showInBar = ref(initial.showInBar)

  function persist() {
    try {
      localStorage.setItem(
        KEY,
        JSON.stringify({
          fontSize: fontSize.value,
          offsetMs: offsetMs.value,
          mode: mode.value,
          theme: theme.value,
          showInBar: showInBar.value,
        }),
      )
    } catch {
      // 存不下就只在本次会话生效
    }
  }

  function setFontSize(next: number) {
    fontSize.value = clampFont(next)
    persist()
  }

  function setOffset(next: number) {
    offsetMs.value = clampOffset(next)
    persist()
  }

  /** 往「歌词更晚」方向挪一格；符号与界面文案一致：正值 = 歌词出现得更晚 */
  function nudgeOffset(steps: number) {
    setOffset(offsetMs.value + steps * LYRIC_OFFSET_STEP)
  }

  function setMode(next: LyricMode) {
    mode.value = next
    persist()
  }

  function toggleMode() {
    setMode(mode.value === 'cover' ? 'pure' : 'cover')
  }

  function setTheme(next: LyricThemeId) {
    theme.value = next
    persist()
  }

  function setShowInBar(next: boolean) {
    showInBar.value = next
    persist()
  }

  const canNudgeLater = computed(() => offsetMs.value < LYRIC_OFFSET_LIMIT)
  const canNudgeEarlier = computed(() => offsetMs.value > -LYRIC_OFFSET_LIMIT)

  return {
    fontSize,
    offsetMs,
    mode,
    theme,
    showInBar,
    canNudgeLater,
    canNudgeEarlier,
    setFontSize,
    setOffset,
    nudgeOffset,
    setMode,
    toggleMode,
    setTheme,
    setShowInBar,
  }
})
