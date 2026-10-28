export type SourceId = 'netease' | 'qq' | 'kuwo'
export type Fee = 'free' | 'vip' | 'paid' | 'unknown'

/** 界面强调色（主题色），与明暗主题正交 */
export type AccentId = 'violet' | 'indigo' | 'teal' | 'rose' | 'amber' | 'green'

/**
 * 跨平台对齐的音质等级。只表达「谁比谁高」，不承诺各平台同等级音质相同——
 * 具体落到哪一档由各平台自己的档位表决定（见 {@link QualityTier}）。
 */
export type QualityLevel = 'standard' | 'high' | 'exquisite' | 'lossless' | 'hiRes' | 'master'

/** 某个平台上的具体音质档位，名称照抄平台自己的叫法 */
export interface QualityTier {
  /** 平台内部档位码，仅后端解析器使用 */
  code: string
  label: string
  /** 副标题：格式与规格，例如「FLAC 24bit/96kHz」 */
  detail: string
  level: QualityLevel
  /** 归一化码率，无损按 1000 记，仅用于展示 */
  bitrate: number
  format: string
  /** 通常需要会员 */
  vip: boolean
}

/** 一个平台的完整档位表，从高到低 */
export interface PlatformQuality {
  source: SourceId
  label: string
  /** 平台级说明，例如「该接口只提供标准音质」 */
  note: string
  items: QualityTier[]
}

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
  | 'linkRejected'
  | 'invalid'
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
  /** 平台给的人气值（网易云 0–100，其它源可能没有 → 0） */
  popularity?: number
  /** 平台标注的原唱/翻唱（目前只有网易云给），没有就是 undefined */
  origin?: OriginMark | null
}

/** 平台侧的原唱 / 翻唱标注 */
export type OriginMark = 'original' | 'cover'

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
  /** 各音源人气值里的最大值，仅用于「按热度排序」 */
  popularity?: number
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
  /** 已经挑好的地址：能直连就是原链，否则是本地代理 */
  url: string
  /** 音源原始直链 */
  directUrl?: string
  /** 补了 Referer 的代理地址，直连失败时换这个 */
  proxyUrl?: string | null
  bitrate: number
  format: string
  size: number
}

/** 播放失败的结构化原因，由内核直接给文案 */
export interface PlayFailure {
  reason: FailReason
  label: string
  message: string
  hint: string
  source: SourceId
}

export interface Lyric {
  lrc: string
  translation: string | null
}

/** 「我的歌单」列表项（摘要，不含曲目） */
export interface PlaylistSummary {
  source: SourceId
  id: string
  name: string
  cover: string | null
  trackCount: number
  creator: string | null
}

/** 歌单详情：点进去才拉曲目 */
export interface PlaylistDetail {
  source: SourceId
  id: string
  name: string
  cover: string | null
  total: number
  items: Variant[]
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
  /** 入队时选的目标音质 */
  quality: QualityLevel
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
  quality?: QualityLevel
  format?: string
  fallbacks?: Fallback[]
}

export interface Settings {
  downloadDir: string
  /** 全局默认目标音质 */
  quality: QualityLevel
  /** 分平台覆盖：某个音源单独指定目标音质 */
  qualityOverrides: Partial<Record<SourceId, QualityLevel>>
  /** 目标档位取不到时是否自动降级 */
  autoFallback: boolean
  /** 配置结构版本，由后端维护 */
  schema: number
  preferSource: SourceId | null
  namingTemplate: string
  artistSeparator: string
  concurrency: number
  /** 单任务下载限速（KB/s），0 表示不限 */
  speedLimitKbps: number
  /** 全局代理地址（http/https/socks5），空字符串 = 直连 */
  proxy: string
  writeTags: boolean
  embedCover: boolean
  saveLyric: boolean
  duplicatePolicy: 'skip' | 'overwrite' | 'keepBoth'
  theme: 'light' | 'dark' | 'system'
  accent: AccentId
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
