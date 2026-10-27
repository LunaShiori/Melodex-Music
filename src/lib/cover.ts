import { proceduralCover } from './artwork'

/**
 * `<img>` 加载失败时的兜底。
 *
 * 封面来自第三方 CDN，随时可能 404 / 403 / 被墙，加载失败就是个空白方块，
 * 看起来像「封面坏了」。这里换成本地生成的程序化渐变封面（同一首歌始终是同一张，
 * 不会跳来跳去），保证任何情况下都有东西可看。
 *
 * 用法：`<img :src="cover" @error="onCoverError($event, track.fingerprint)" />`
 */
export function onCoverError(event: Event, seed: string): void {
  const image = event.target as HTMLImageElement | null
  if (!image) return
  const fallback = proceduralCover(seed || 'melodex')
  // 兜底图本身也可能因为 data URI 解析失败再触发一次 error，防一下死循环
  if (image.src !== fallback) image.src = fallback
}
