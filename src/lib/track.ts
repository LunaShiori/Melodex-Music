import type { Fallback, SourceId, Track, Variant } from './types'

/** 挑选下载/播放用的具体记录：指定音源优先 > 免费优先 > 音质最高。 */
export function bestVariant(track: Track, prefer?: SourceId | null): Variant {
  const pool = track.variants.length ? track.variants : []
  if (!pool.length) {
    throw new Error('该条目没有可用音源')
  }
  const rankFee = (fee: Variant['fee']) =>
    fee === 'free' ? 0 : fee === 'unknown' ? 1 : fee === 'vip' ? 2 : 3
  return [...pool].sort((a, b) => {
    const sourceRank = (v: Variant) => (prefer && v.source === prefer ? 0 : 1)
    return (
      sourceRank(a) - sourceRank(b) || rankFee(a.fee) - rankFee(b.fee) || b.bitrate - a.bitrate
    )
  })[0]
}

/**
 * 其它音源上的候选，按「免费优先 > 音质高」排序，交给引擎在首选源失败时依次尝试。
 *
 * 一首歌在 QQ 音乐要会员、在酷我免费是常态，多带一份候选就能少一次「下不了」。
 */
export function fallbacksOf(track: Track, primary: SourceId): Fallback[] {
  const rankFee = (fee: Variant['fee']) =>
    fee === 'free' ? 0 : fee === 'unknown' ? 1 : fee === 'vip' ? 2 : 3
  return track.variants
    .filter((variant) => variant.source !== primary)
    .sort((a, b) => rankFee(a.fee) - rankFee(b.fee) || b.bitrate - a.bitrate)
    .map((variant) => ({
      source: variant.source,
      sourceId: variant.id,
      bitrate: variant.bitrate,
    }))
}

export function variantOf(track: Track, source: SourceId): Variant | undefined {
  return track.variants.find((variant) => variant.source === source)
}

export function artistLine(artists: string[], separator = ' / '): string {
  return artists.filter(Boolean).join(separator) || '未知歌手'
}
