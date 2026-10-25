export type SourceId = 'netease' | 'qq' | 'kuwo'
export type Fee = 'free' | 'vip' | 'paid' | 'unknown'

/**
 * 下载失败的结构化原因。界面据此决定显示「去登录」还是「重试」，
 * 而不是笼统地说一句「该歌曲没有可用的音频链接」。
 */
export type FailReason =
  | 'needLogin'
  | 'vipOnly'
  | 'qualityMissing'
  | 'copyright'
  | 'network'
  | 'unknown'

export interface Variant {
  source: SourceId
  id: string
  title: string
  artists: string[]
  album: string
  durationMs: number
  cover: string | null
  bitrate: number
  format: string
  size: number
  fee: Fee
}

export interface Track {
  fingerprint: string
  title: string
  artists: string[]
  album: string
  durationMs: number
  cover: string | null
  bitrate: number
  format: string
  fee: Fee
  variants: Variant[]
}

export interface SourceReport {
  source: SourceId
  ok: boolean
  count: number
  elapsedMs: number
  error: string | null
}

export interface AggregatedPage {
  keyword: string
  page: number
  tracks: Track[]
  reports: SourceReport[]
}

export interface PlayUrl {
  url: string
  bitrate: number
  format: string
  size: number
}

export interface Lyric {
  lrc: string
  translation: string | null
}

export type TaskState =
  | 'queued'
  | 'resolving'
  | 'downloading'
  | 'tagging'
  | 'completed'
  | 'failed'
  | 'paused'
  | 'canceled'

export interface DownloadTask {
  id: string
  title: string
  artists: string[]
  album: string
  cover: string | null
  source: SourceId
  sourceId: string
  bitrate: number
  format: string
  durationMs: number
  state: TaskState
  downloaded: number
  total: number
  speedBps: number
  path: string | null
  error: string | null
  /** 失败原因，失败时才有值 */
  failReason: FailReason | null
  /** 其它音源上的候选，首选源失败时引擎会自动换源 */
  fallbacks: Fallback[]
  /** 最终真正取到链接的音源，可能和 source 不同 */
  resolvedSource: SourceId | null
  createdAt: number
  finishedAt: number | null
}

export interface Fallback {
  source: SourceId
  sourceId: string
  bitrate: number
}

export interface Notice {
  level: 'info' | 'success' | 'warn' | 'error'
  message: string
}

export type EngineEvent =
  | { type: 'taskUpsert'; payload: DownloadTask }
  | { type: 'taskRemoved'; payload: string }
  | { type: 'notice'; payload: Notice }

export interface EnqueueInput {
  source: SourceId
  sourceId: string
  title: string
  artists: string[]
  album: string
  cover: string | null
  durationMs: number
  bitrate?: number
  format?: string
  fallbacks?: Fallback[]
}

export interface Settings {
  downloadDir: string
  bitrate: number
  fallbackBitrates: number[]
  preferSource: SourceId | null
  namingTemplate: string
  artistSeparator: string
  concurrency: number
  writeTags: boolean
  embedCover: boolean
  saveLyric: boolean
  duplicatePolicy: 'skip' | 'overwrite' | 'keepBoth'
  theme: 'light' | 'dark' | 'system'
  sources: SourceId[]
}

export const SOURCE_META: Record<
  SourceId,
  { label: string; short: string; color: string; home: string }
> = {
  netease: { label: '网易云音乐', short: '网易云', color: '#e23b3b', home: 'https://music.163.com' },
  qq: { label: 'QQ 音乐', short: 'QQ', color: '#23a166', home: 'https://y.qq.com' },
  kuwo: { label: '酷我音乐', short: '酷我', color: '#ef9f1f', home: 'https://www.kuwo.cn' },
}

export const ALL_SOURCES: SourceId[] = ['netease', 'qq', 'kuwo']

/** 发布清单里的一个可下载文件（安装包 / 压缩包） */
export interface ReleaseAsset {
  name: string
  size: number
  url: string
  /** nsis | msi | apk | aab | dmg | appimage | deb | zip | other */
  kind: string
}

/** 更新检查结果 */
export interface UpdateInfo {
  current: string
  latest: string
  available: boolean
  notes: string
  pageUrl: string
  publishedAt: string
  assets: ReleaseAsset[]
}

/** 安装包下载进度，由内核推送 */
export interface UpdateProgress {
  downloaded: number
  total: number
}

/* ---------- 账号与本地加密凭据 ---------- */

/** 已保存的账号摘要（不含 Cookie 明文） */
export interface AccountInfo {
  source: SourceId
  nickname: string | null
  savedAt: number
  /** Cookie 里包含哪些键，例如 MUSIC_U / __csrf */
  keys: string[]
}

/** 凭据库状态 */
export interface VaultStatus {
  exists: boolean
  pinProtected: boolean
  unlocked: boolean
  accounts: AccountInfo[]
  minPinLen: number
}

/** 扫码登录票据 */
export interface QrTicket {
  source: SourceId
  key: string
  /** 编码进二维码的内容；QQ 的二维码是位图，这里为空串 */
  qrUrl: string
  expiresIn: number
  /** 本地渲染好的 SVG（网易云），直接用 v-html 内联 */
  qrSvg: string | null
  /** 音源直接下发的位图（QQ），形如 data:image/png;base64,... */
  qrImage: string | null
}

export type QrStatus =
  | 'waiting'
  | 'scanned'
  | 'confirmed'
  | 'expired'
  /** 登录链路本身出错，`message` 里是具体原因 */
  | 'error'

/** 一次轮询结果 */
export interface QrPoll {
  status: QrStatus
  cookie: string | null
  nickname: string | null
  message: string | null
}
