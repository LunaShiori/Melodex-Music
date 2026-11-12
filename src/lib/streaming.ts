/**
 * 外部平台的可用边界。
 *
 * 这里只做「能做的事」：Apple Music 用公开的 iTunes Search API 搜歌和试听 30 秒片段，
 * 全曲受 FairPlay DRM 保护，没有明文直链，播放和下载都不做；Spotify 的 Web API 需要
 * 开发者凭据，只做元数据检索与跳转；YouTube Music 和 B 站要么没有公开接口、要么需要
 * 自备 key，这里只登记边界，不做假装能用的按钮。
 */

export type PlatformId = 'apple' | 'spotify' | 'youtube' | 'bilibili'

/** 三档能力：能直接用 / 需要用户自备凭据 / 这条路走不通 */
export type PlatformTier = 'ready' | 'credentials' | 'blocked'

export interface PlatformCapability {
  id: PlatformId
  tier: PlatformTier
  /** 首页地址，界面上给一个「去官网看看」 */
  home: string
  /** 搜索入口模板，{q} 会被替换成关键词 */
  searchUrl?: string
}

export const PLATFORM_CAPABILITIES: PlatformCapability[] = [
  {
    id: 'apple',
    tier: 'ready',
    home: 'https://music.apple.com/',
    searchUrl: 'https://music.apple.com/search?term={q}',
  },
  {
    id: 'spotify',
    tier: 'credentials',
    home: 'https://open.spotify.com/',
    searchUrl: 'https://open.spotify.com/search/{q}',
  },
  {
    id: 'youtube',
    tier: 'blocked',
    home: 'https://music.youtube.com/',
  },
  {
    id: 'bilibili',
    tier: 'blocked',
    home: 'https://www.bilibili.com/v/music/',
  },
]

/** iTunes 的店铺地区。中国大陆店的曲库偏少，留几个常用的让用户自己切 */
export const APPLE_STORES = [
  { id: 'cn', label: '中国大陆' },
  { id: 'us', label: 'United States' },
  { id: 'jp', label: 'Japan' },
  { id: 'hk', label: 'Hong Kong' },
  { id: 'tw', label: 'Taiwan' },
] as const

export type AppleStore = (typeof APPLE_STORES)[number]['id']

export interface AppleTrack {
  id: number
  title: string
  artist: string
  album: string
  artwork: string | null
  durationMs: number
  /** 30 秒试听片段；没有的话说明这首不给试听 */
  previewUrl: string | null
  /** apple music 网页版地址，用来「在 Apple Music 里打开」 */
  url: string
}

interface RawAppleTrack {
  trackId?: number
  trackName?: string
  artistName?: string
  collectionName?: string
  artworkUrl100?: string
  trackTimeMillis?: number
  previewUrl?: string
  trackViewUrl?: string
}

/**
 * 搜 Apple Music。
 *
 * 直接用浏览器的 fetch 打 iTunes 的公开接口（它回 `Access-Control-Allow-Origin: *`），
 * 不经过内核，也不需要任何密钥。店铺地区会影响曲库，所以结果和 store 一起返回。
 */
export async function searchAppleMusic(
  term: string,
  store: AppleStore,
  signal?: AbortSignal,
): Promise<AppleTrack[]> {
  const keyword = term.trim()
  if (!keyword) return []
  const url =
    'https://itunes.apple.com/search?media=music&entity=song&limit=25' +
    `&country=${encodeURIComponent(store)}&term=${encodeURIComponent(keyword)}`
  const response = await fetch(url, { signal })
  if (!response.ok) throw new Error(`iTunes 返回 HTTP ${response.status}`)
  const payload = (await response.json()) as { results?: RawAppleTrack[] }
  const rows = payload.results ?? []
  return rows
    .filter((row): row is RawAppleTrack & { trackId: number } => typeof row.trackId === 'number')
    .map((row) => ({
      id: row.trackId,
      title: row.trackName ?? '',
      artist: row.artistName ?? '',
      album: row.collectionName ?? '',
      // 100px 的封面换成 300px，列表里够用且不用二次请求
      artwork: row.artworkUrl100 ? row.artworkUrl100.replace('100x100', '300x300') : null,
      durationMs: row.trackTimeMillis ?? 0,
      previewUrl: row.previewUrl ?? null,
      url: row.trackViewUrl ?? '',
    }))
}
