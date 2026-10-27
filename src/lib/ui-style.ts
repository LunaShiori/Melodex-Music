/**
 * 界面风格（material）。
 *
 * 与「明暗主题」和「强调色」是三个正交维度：
 * - 明暗主题决定底色深浅（light / dark）
 * - 强调色决定主色（violet / indigo / ...）
 * - 界面风格决定**质感**：圆角大小、模糊程度、描边与阴影（本文件）
 *
 * `classic` 不写任何 CSS 覆盖，所以升级后老用户看到的界面和以前一模一样；
 * 其余风格是叠加在变量层的预设，想换随时换回来。
 *
 * 偏好只存 localStorage 而不进设置文件：这是纯外观选择，
 * 和「曲库视图偏好」一样属于设备级偏好，没必要为它改后端设置 schema。
 */

export type UiStyleId = 'classic' | 'glass' | 'aurora' | 'minimal'

export interface UiStyleDef {
  id: UiStyleId
  label: string
  desc: string
  /** 选择器上的一小块预览渐变 */
  preview: [string, string]
}

export const UI_STYLES: UiStyleDef[] = [
  {
    id: 'classic',
    label: '经典',
    desc: '原本的样子，克制的卡片与阴影',
    preview: ['#1b1b24', '#6b5bf0'],
  },
  {
    id: 'glass',
    label: '液态玻璃',
    desc: '半透明毛玻璃、大圆角、边缘高光',
    preview: ['#dbe6ff', '#9fb6ff'],
  },
  {
    id: 'aurora',
    label: '极光科技',
    desc: '深空底色、霓虹描边、流动光晕',
    preview: ['#05070f', '#22d3ee'],
  },
  {
    id: 'minimal',
    label: '极简',
    desc: '去阴影去渐变，细线高对比',
    preview: ['#ffffff', '#111318'],
  },
]

const STYLE_KEY = 'melodex.ui.style'
const DEBUG_KEY = 'melodex.ui.debug'

export function isUiStyle(value: unknown): value is UiStyleId {
  return UI_STYLES.some((item) => item.id === value)
}

export function readUiStyle(): UiStyleId {
  try {
    const stored = localStorage.getItem(STYLE_KEY)
    if (isUiStyle(stored)) return stored
  } catch {
    /* localStorage 不可用时按下面的默认值走 */
  }
  // 没手动选过时按设备给默认值：手机端本身已经在用毛玻璃的底栏和播放条，
  // 整体切成液态玻璃更统一；桌面端保持原有观感，想换自己去设置里切。
  try {
    if (window.matchMedia('(max-width: 860px)').matches) return 'glass'
  } catch {
    /* 老 WebView 没有 matchMedia */
  }
  return 'classic'
}

export function writeUiStyle(id: UiStyleId): void {
  try {
    localStorage.setItem(STYLE_KEY, id)
  } catch {
    /* 隐私模式下写不进去，界面照样能用 */
  }
}

export function readDebugMode(): boolean {
  try {
    return localStorage.getItem(DEBUG_KEY) === '1'
  } catch {
    return false
  }
}

export function writeDebugMode(on: boolean): void {
  try {
    localStorage.setItem(DEBUG_KEY, on ? '1' : '0')
  } catch {
    /* 同上 */
  }
}
