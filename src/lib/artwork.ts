/** 程序化封面：无网络依赖，按种子生成稳定的渐变唱片图。 */

const PALETTES: Array<[string, string, string]> = [
  ['#6b5bf0', '#a78bfa', '#22d3ee'],
  ['#f97362', '#fbbf24', '#6b5bf0'],
  ['#0ea5e9', '#22d3ee', '#a3e635'],
  ['#e11d48', '#fb7185', '#fda4af'],
  ['#10b981', '#34d399', '#a7f3d0'],
  ['#8b5cf6', '#ec4899', '#f59e0b'],
  ['#0f172a', '#334155', '#94a3b8'],
  ['#f59e0b', '#fbbf24', '#fde68a'],
]

function hash(seed: string): number {
  let value = 2166136261
  for (let i = 0; i < seed.length; i += 1) {
    value ^= seed.charCodeAt(i)
    value = Math.imul(value, 16777619)
  }
  return Math.abs(value)
}

export function proceduralCover(seed: string): string {
  const value = hash(seed)
  const palette = PALETTES[value % PALETTES.length]
  const angle = value % 360
  const cx = 30 + (value % 40)
  const cy = 30 + ((value >> 3) % 40)
  const r = 18 + ((value >> 6) % 16)
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
    <defs>
      <linearGradient id="g" gradientTransform="rotate(${angle} 0.5 0.5)">
        <stop offset="0%" stop-color="${palette[0]}"/>
        <stop offset="55%" stop-color="${palette[1]}"/>
        <stop offset="100%" stop-color="${palette[2]}"/>
      </linearGradient>
    </defs>
    <rect width="100" height="100" fill="url(#g)"/>
    <circle cx="${cx}" cy="${cy}" r="${r}" fill="none" stroke="#ffffff" stroke-opacity="0.35" stroke-width="0.8"/>
    <circle cx="${cx}" cy="${cy}" r="${r * 0.6}" fill="none" stroke="#ffffff" stroke-opacity="0.24" stroke-width="0.8"/>
    <circle cx="${cx}" cy="${cy}" r="${r * 0.22}" fill="#ffffff" fill-opacity="0.5"/>
    <path d="M0 ${80 + (value % 12)} Q 25 ${70 + (value % 20)} 50 ${84 + (value % 8)} T 100 ${76 + (value % 14)}" fill="none" stroke="#ffffff" stroke-opacity="0.18" stroke-width="1.2"/>
  </svg>`
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
}

/**
 * 把封面地址规整成能直接加载的形式。
 *
 * QQ 的老接口（歌单详情 `logo`）会回 `http://y.gtimg.cn/...`。移动端 WebView
 * 和打包后的桌面端页面运行在 https 安全上下文里，http 图片会被当成混合内容
 * 直接拦掉，`<img>` 一片空白——看起来像「封面读不出来」。统一升级成 https。
 */
export function normalizeCover(url: string): string {
  if (url.startsWith('http://')) return `https://${url.slice(7)}`
  return url
}

export function coverOf(cover: string | null | undefined, seed: string): string {
  if (cover && /^https?:\/\//.test(cover)) return normalizeCover(cover)
  // 有些接口给的是协议无关地址（//host/a.jpg），补上 https 才能加载
  if (cover && cover.startsWith('//')) return `https:${cover}`
  return proceduralCover(seed || 'melodex')
}
