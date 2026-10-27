/**
 * 「原唱 / 翻唱」识别。
 *
 * 各平台都没有一个可靠的统一字段：网易云有 `originCoverType`，QQ 和酷我没有，
 * 但用户看到的其实是**标题里的自述**——「晴天（原唱：周杰伦）」「起风了 cover 版」。
 * 所以这里按标题文本判定，口径和用户看到的一致：标题里写着「原唱」的就算原唱标注。
 *
 * 只用于列表排序和打标，不参与任何下载/解析逻辑——判断错了顶多顺序不好看。
 */

/** 标题自带「这就是原唱 / 原版」的说明 */
const ORIGINAL_PATTERN = /原唱|原版|原曲|原声/

/** 翻唱、改编、现场等版本标记（中英混排，英文按词边界匹配，避免 olive 之类误伤） */
const COVER_PATTERN =
  /翻唱|翻自|改编|重制|伴奏|纯伴奏|女生版|女声版|男声版|童声版|抖音版|加速版|慢速版|降调|升调|片段|铃声|现场版|钢琴版|吉他版|古筝版|(^|[^a-z])cover([^a-z]|$)|(^|[^a-z])live([^a-z]|$)|(^|[^a-z])remix([^a-z]|$)|(^|[^a-z])dj([^a-z]|$)|(^|[^a-z])inst(\.|rumental)?([^a-z]|$)/i

export type OriginKind = 'original' | 'cover' | 'unknown'

/**
 * 判定一首歌是原唱标注、翻唱版本，还是看不出来。
 *
 * 顺序很重要：「原唱」优先——「晴天（原唱：周杰伦）Cover 版」这种标题两种词都有，
 * 用户关心的仍然是不是原唱。
 */
export function originOf(input: { title: string; album?: string | null }): OriginKind {
  const text = `${input.title} ${input.album ?? ''}`
  if (ORIGINAL_PATTERN.test(text)) return 'original'
  if (COVER_PATTERN.test(text)) return 'cover'
  return 'unknown'
}

/** 是否算「原唱」——置顶原唱就是按这个合集筛的 */
export function isOriginal(input: { title: string; album?: string | null }): boolean {
  return originOf(input) === 'original'
}

/** 标题里能看出的版本说明，用于列表上的小标签；看不出来就返回空串 */
export function originLabel(input: { title: string; album?: string | null }): string {
  const kind = originOf(input)
  if (kind === 'original') return '原唱'
  if (kind === 'cover') return '翻唱'
  return ''
}
