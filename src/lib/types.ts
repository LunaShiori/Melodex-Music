export type SourceId = 'netease' | 'qq' | 'kuwo'
export type Fee = 'free' | 'vip' | 'paid' | 'unknown'

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
  createdAt: number
  finishedAt: number | null
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
