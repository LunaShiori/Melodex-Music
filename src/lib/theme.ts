import type { AccentId } from './types'

/**
 * 界面强调色（主题色）目录。每个强调色给出主色、浅色、以及按下/文字用的对比色。
 * 主色注入 `--accent`，浅色注入 `--accent-soft`，配合明暗主题形成「多源主题」。
 */
export interface AccentDef {
  id: AccentId
  label: string
  /** 主强调色 */
  accent: string
  /** hover/渐变用的浅一档 */
  soft: string
  /** 强调色上的文字颜色（按钮前景） */
  ink: string
}

export const ACCENTS: AccentDef[] = [
  { id: 'violet', label: '紫罗兰', accent: '#6b5bf0', soft: '#8f82ff', ink: '#ffffff' },
  { id: 'indigo', label: '靛蓝', accent: '#3d6bf0', soft: '#6f96ff', ink: '#ffffff' },
  { id: 'teal', label: '青碧', accent: '#0d9e8c', soft: '#2fc7b2', ink: '#ffffff' },
  { id: 'rose', label: '玫红', accent: '#e0447a', soft: '#f4739e', ink: '#ffffff' },
  { id: 'amber', label: '琥珀', accent: '#e08a18', soft: '#f2ac47', ink: '#1a1206' },
  { id: 'green', label: '翠绿', accent: '#1fa86a', soft: '#43c98a', ink: '#ffffff' },
]

export function accentOf(id: AccentId): AccentDef {
  return ACCENTS.find((item) => item.id === id) ?? ACCENTS[0]
}
