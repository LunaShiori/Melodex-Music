/**
 * 「原唱 / 翻唱」识别。
 *
 * 分两层，谁更可信用谁：
 *
 * 1. **平台标注**——网易云的搜索接口每条都带 `originCoverType`（1 = 原唱，2 = 翻唱），
 *    这是平台自己的判定，比猜标题准得多。0 和 3 都当「没标」，实测 3 里混着翻唱。
 * 2. **标题文本**——QQ、酷我没有对应字段，只能看标题里的自述：
 *    「晴天（原唱：周杰伦）」「起风了 cover 版」。
 *
 * 只用于列表上的小标签，不参与任何排序、下载或解析逻辑——判断错了顶多标签不好看。
 */

import type { OriginMark } from './types'

/** 标题自带「这就是原唱 / 原版」的说明 */
const ORIGINAL_PATTERN = /原唱|原版|原曲|原声/

/** 翻唱、改编、现场等版本标记（中英混排，英文按词边界匹配，避免 olive 之类误伤） */
const COVER_PATTERN =
  /翻唱|翻自|改编|重制|伴奏|纯伴奏|女生版|女声版|男声版|童声版|抖音版|加速版|慢速版|降调|升调|片段|铃声|现场版|钢琴版|吉他版|古筝版|(^|[^a-z])cover([^a-z]|$)|(^|[^a-z])live([^a-z]|$)|(^|[^a-z])remix([^a-z]|$)|(^|[^a-z])dj([^a-z]|$)|(^|[^a-z])inst(\.|rumental)?([^a-z]|$)/i

export type OriginKind = 'original' | 'cover' | 'unknown'

/**
 * 只看文本的判定，顺序很重要：「原唱」优先——
 * 「晴天（原唱：周杰伦）Cover 版」这种标题两种词都有，用户关心的仍然是不是原唱。
 */
export function originOf(input: { title: string; album?: string | null }): OriginKind {
  const text = `${input.title} ${input.album ?? ''}`
  if (ORIGINAL_PATTERN.test(text)) return 'original'
  if (COVER_PATTERN.test(text)) return 'cover'
  return 'unknown'
}

/**
 * 一首歌（合并后的条目）的版本口径。
 *
 * 只要有一个音源被平台标成原唱就算原唱——同一条记录在「原唱源」上的判定最权威；
 * 各源都没标才退回标题文本。
 */
export function trackOrigin(track: {
  title: string
  album?: string | null
  variants?: { origin?: OriginMark | null }[]
}): OriginKind {
  const marks = (track.variants ?? [])
    .map((variant) => variant.origin)
    .filter((mark): mark is OriginMark => mark === 'original' || mark === 'cover')
  if (marks.includes('original')) return 'original'
  if (marks.includes('cover')) return 'cover'
  return originOf(track)
}

/** 标题里能看出的版本说明，用于列表上的小标签；看不出来就返回空串 */
export function originLabel(track: {
  title: string
  album?: string | null
  variants?: { origin?: OriginMark | null }[]
}): string {
  const kind = trackOrigin(track)
  if (kind === 'original') return '原唱'
  if (kind === 'cover') return '翻唱'
  return ''
}
