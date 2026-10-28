/**
 * 「原唱 / 翻唱」标签。
 *
 * **只认平台自己的判定**。网易云的搜索接口每条都带 `originCoverType`
 * （1 = 原唱，2 = 翻唱），这是平台给出的，比猜标题准得多；0 和 3 都当「没标」，
 * 实测 3 里混着翻唱，用了反而误导。
 *
 * QQ 音乐和酷我没有对应字段，所以它们**不显示**这项标签——
 * 宁可不标，也不拿标题里的「Cover」「原唱」去猜：歌名是用户自己起的，
 * 猜错了比不标更招人烦。
 *
 * 只用于列表上的小标签，不参与任何排序、下载或解析逻辑。
 */

import type { OriginMark } from './types'

export type OriginKind = 'original' | 'cover' | 'unknown'

/**
 * 一首歌（合并后的条目）的版本口径。
 *
 * 只要有一个音源被平台标成原唱就算原唱——同一条记录在「原唱源」上的判定最权威；
 * 各源都没标就是 `unknown`，界面上不显示。
 */
export function trackOrigin(track: {
  variants?: { origin?: OriginMark | null }[]
}): OriginKind {
  const marks = (track.variants ?? [])
    .map((variant) => variant.origin)
    .filter((mark): mark is OriginMark => mark === 'original' || mark === 'cover')
  if (marks.includes('original')) return 'original'
  if (marks.includes('cover')) return 'cover'
  return 'unknown'
}

/** 列表上的小标签；平台没标就返回空串 */
export function originLabel(track: { variants?: { origin?: OriginMark | null }[] }): string {
  const kind = trackOrigin(track)
  if (kind === 'original') return '原唱'
  if (kind === 'cover') return '翻唱'
  return ''
}
