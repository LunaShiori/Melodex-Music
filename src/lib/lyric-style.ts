/**
 * 歌词页的显示方式：模式 × 主题。
 *
 * 两件事分开管：
 * - **模式**决定版面（只有歌词 / 封面加歌词）
 * - **主题**决定质感（底色、发光、字体气质）
 *
 * 两者正交，可以随便组合。全部靠 CSS 变量 + `data-mode` / `data-theme` 驱动，
 * 组件里不出现硬编码颜色，明暗主题切换时自动跟着走。
 */

/** 只有歌词 / 封面 + 歌词 */
export type LyricMode = 'pure' | 'cover'

export interface LyricModeDef {
  id: LyricMode
  label: string
  desc: string
  /** 工具栏图标名，走 Icon.vue 的那套线条图标 */
  icon: string
}

export const LYRIC_MODES: LyricModeDef[] = [
  {
    id: 'cover',
    label: '歌词 + 封面',
    desc: '封面在上、歌词在下，桌面端左右分栏',
    icon: 'disc',
  },
  {
    id: 'pure',
    label: '纯歌词',
    desc: '只留歌词，屏幕全给文字',
    icon: 'lyric',
  },
]

export type LyricThemeId = 'aurora' | 'glass' | 'vinyl' | 'ink'

export interface LyricThemeDef {
  id: LyricThemeId
  label: string
  desc: string
  /** 主题缩略图的两个渐变色 */
  preview: [string, string]
  /** 是否整体走浅色底（暗色主题下也保持纸面观感） */
  light?: boolean
}

export const LYRIC_THEMES: LyricThemeDef[] = [
  {
    id: 'aurora',
    label: '极光',
    desc: '封面放大成背景光晕，当前行发光',
    preview: ['#0b1220', '#7c6cff'],
  },
  {
    id: 'glass',
    label: '玻璃',
    desc: '浅色磨砂 + 主题色光斑，白天更清爽',
    preview: ['#eef2ff', '#c7d3ff'],
    light: true,
  },
  {
    id: 'vinyl',
    label: '黑胶',
    desc: '唱片旋转，暖色高亮，像一台老唱机',
    preview: ['#07070a', '#e0a94f'],
  },
  {
    id: 'ink',
    label: '词卡',
    desc: '纸面衬线字，歌词像印在卡片上',
    preview: ['#f6f1e7', '#3d3325'],
    light: true,
  },
]

export const LYRIC_FONT_MIN = 14
export const LYRIC_FONT_MAX = 34
export const LYRIC_FONT_DEFAULT = 22

export const LYRIC_OFFSET_LIMIT = 3000
/** 偏移每次点一下走多少毫秒 */
export const LYRIC_OFFSET_STEP = 100

export function isLyricMode(value: unknown): value is LyricMode {
  return LYRIC_MODES.some((item) => item.id === value)
}

export function isLyricTheme(value: unknown): value is LyricThemeId {
  return LYRIC_THEMES.some((item) => item.id === value)
}

/** 把任意输入夹到合法区间，避免老配置把字号撑爆 */
export function clampFont(value: number): number {
  if (!Number.isFinite(value)) return LYRIC_FONT_DEFAULT
  return Math.min(LYRIC_FONT_MAX, Math.max(LYRIC_FONT_MIN, Math.round(value)))
}

export function clampOffset(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(LYRIC_OFFSET_LIMIT, Math.max(-LYRIC_OFFSET_LIMIT, Math.round(value / 50) * 50))
}

/** 偏移的人话形式：`+0.3s` / `-1.2s` / `0.0s` */
export function formatOffset(ms: number): string {
  const seconds = ms / 1000
  const sign = seconds > 0 ? '+' : ''
  return `${sign}${seconds.toFixed(1)}s`
}
