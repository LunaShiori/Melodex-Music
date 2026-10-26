import type { QualityLevel } from './types'

/**
 * 六个音质等级，从低到高。
 *
 * 这是**跨平台**的比较维度：各平台档位名称不同（QQ 的 SQ、网易的无损音质），
 * 但都需要回答「这个平台最高能给到哪一档」。具体档位表由后端下发。
 */
export const QUALITY_LEVELS: { id: QualityLevel; label: string }[] = [
  { id: 'standard', label: '标准' },
  { id: 'high', label: '较高' },
  { id: 'exquisite', label: '极高' },
  { id: 'lossless', label: '无损' },
  { id: 'hiRes', label: 'Hi-Res' },
  { id: 'master', label: '母带' },
]

/** 等级序号，用于「不高于某等级」这类比较 */
export function qualityRank(level: QualityLevel): number {
  const index = QUALITY_LEVELS.findIndex((item) => item.id === level)
  return index < 0 ? 0 : index
}

/** 等级对应的标称码率，仅用于命名模板预览这类展示场景 */
export function nominalBitrate(level: QualityLevel): number {
  switch (level) {
    case 'master':
      return 3000
    case 'hiRes':
      return 2000
    case 'lossless':
      return 1000
    case 'exquisite':
      return 320
    case 'high':
      return 192
    default:
      return 128
  }
}
