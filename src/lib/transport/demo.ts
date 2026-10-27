import type {
  AccountInfo,
  AggregatedPage,
  DownloadTask,
  EngineEvent,
  EnqueueInput,
  Lyric,
  PlatformQuality,
  PlaylistDetail,
  PlaylistSummary,
  PlayUrl,
  QrPoll,
  QrTicket,
  QualityLevel,
  Settings,
  SourceId,
  UpdateProgress,
  VaultStatus,
  Variant,
} from '../types'
import { ALL_SOURCES } from '../types'
import type { LoginWebEvent, SearchParams, Transport } from './types'

/**
 * 演示传输层：浏览器里没有 Rust 核心，这里用内置曲库 + 模拟进度
 * 把整套交互跑通，方便 UI 走查与截图。正式功能全部由 Tauri 端提供。
 */

/// 更新下载进度的订阅者（演示模式用它模拟内核推送）
const updateListeners = new Set<(progress: UpdateProgress) => void>()

/// 网页登录结果的订阅者
const loginWebHandlers = new Set<(event: LoginWebEvent) => void>()

const demoAccounts_ = new Map<SourceId, AccountInfo>([
  [
    'netease',
    {
      source: 'netease',
      nickname: '演示账号',
      savedAt: Math.floor(Date.now() / 1000) - 3600,
      keys: ['MUSIC_U', '__csrf'],
    },
  ],
])

function demoAccounts(): AccountInfo[] {
  return [...demoAccounts_.values()]
}

let demoQrTicks = 0

/** 演示用的伪二维码：只求视觉相似，真实二维码由 Rust 侧渲染 */
function fakeQrSvg(size = 33): string {
  let seed = 0x9e3779b9
  const cells: string[] = []
  const finder = (x: number, y: number) => {
    cells.push(`<rect x="${x}" y="${y}" width="7" height="7"/>`)
    cells.push(`<rect x="${x + 1}" y="${y + 1}" width="5" height="5" fill="#fff"/>`)
    cells.push(`<rect x="${x + 2}" y="${y + 2}" width="3" height="3"/>`)
  }
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const inFinder =
        (x < 8 && y < 8) || (x > size - 9 && y < 8) || (x < 8 && y > size - 9)
      if (inFinder) continue
      seed = (seed * 1103515245 + 12345) & 0x7fffffff
      if ((seed >> 16) % 100 < 44) {
        cells.push(`<rect x="${x}" y="${y}" width="1" height="1"/>`)
      }
    }
  }
  finder(1, 1)
  finder(size - 8, 1)
  finder(1, size - 8)
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1 -1 ${size + 2} ${size + 2}" shape-rendering="crispEdges"><rect x="-1" y="-1" width="${size + 2}" height="${size + 2}" fill="#fff"/><g fill="#12121a">${cells.join('')}</g></svg>`
}

interface Seed {
  title: string
  artists: string[]
  album: string
  durationMs: number
  fee: Variant['fee']
  sources: SourceId[]
}

const CATALOG: Seed[] = [
  { title: '晴天', artists: ['周杰伦'], album: '叶惠美', durationMs: 269_000, fee: 'vip', sources: ['netease', 'qq', 'kuwo'] },
  { title: '七里香', artists: ['周杰伦'], album: '七里香', durationMs: 299_000, fee: 'vip', sources: ['netease', 'qq'] },
  { title: '稻香', artists: ['周杰伦'], album: '魔杰座', durationMs: 223_000, fee: 'free', sources: ['netease', 'qq', 'kuwo'] },
  { title: '夜曲', artists: ['周杰伦'], album: '十一月的萧邦', durationMs: 226_000, fee: 'vip', sources: ['qq', 'kuwo'] },
  { title: '起风了', artists: ['买辣椒也用券'], album: '起风了', durationMs: 325_000, fee: 'free', sources: ['netease', 'kuwo'] },
  { title: '海底', artists: ['一支榴莲'], album: '海底', durationMs: 256_000, fee: 'free', sources: ['netease', 'qq'] },
  { title: '漠河舞厅', artists: ['柳爽'], album: '漠河舞厅', durationMs: 268_000, fee: 'free', sources: ['netease', 'kuwo'] },
  { title: '光年之外', artists: ['邓紫棋'], album: '光年之外', durationMs: 235_000, fee: 'free', sources: ['netease', 'qq', 'kuwo'] },
  { title: '句号', artists: ['邓紫棋'], album: '摩天动物园', durationMs: 244_000, fee: 'vip', sources: ['qq'] },
  { title: '这世界那么多人', artists: ['莫文蔚'], album: '这世界那么多人', durationMs: 285_000, fee: 'free', sources: ['netease', 'qq'] },
  { title: 'Lemon', artists: ['米津玄師'], album: 'Lemon', durationMs: 255_000, fee: 'free', sources: ['netease', 'qq', 'kuwo'] },
  { title: '夜空中最亮的星', artists: ['逃跑计划'], album: '世界', durationMs: 252_000, fee: 'free', sources: ['netease', 'kuwo'] },
]

const BITRATE: Record<SourceId, number[]> = {
  netease: [320, 192, 128],
  qq: [320, 128],
  kuwo: [320, 128],
}

function buildVariants(seed: Seed, index: number): Variant[] {
  return seed.sources.map((source, order) => ({
    source,
    id: `${source}-${index}-${order}`,
    title: seed.title,
    artists: seed.artists,
    album: seed.album,
    durationMs: seed.durationMs,
    cover: null,
    bitrate: BITRATE[source][0],
    format: source === 'qq' ? 'm4a' : 'mp3',
    size: Math.round((seed.durationMs / 1000) * (BITRATE[source][0] / 8) * 1000),
    fee: seed.fee,
  }))
}

const TRACKS = CATALOG.map((seed, index) => {
  const variants = buildVariants(seed, index)
  return {
    fingerprint: `${seed.title}|${seed.artists[0]}|${Math.round(seed.durationMs / 3000)}`,
    title: seed.title,
    artists: seed.artists,
    album: seed.album,
    durationMs: seed.durationMs,
    cover: null,
    bitrate: Math.max(...variants.map((v) => v.bitrate)),
    format: variants[0].format,
    fee: seed.fee,
    variants,
  }
})

const DEMO_LYRIC = `[00:00.00]Melodex 演示模式
[00:06.00]这里展示的是歌词滚动效果
[00:12.00]正式版本会从音源拉取真实歌词
[00:18.00]支持翻译逐行对齐
[00:24.00]故事的小黄花
[00:29.00]从出生那年就飘着
[00:35.00]童年的荡秋千
[00:41.00]随记忆一直晃到现在
[00:48.00]吹着前奏望着天空
[00:54.00]我想起花瓣试着掉落
[01:02.00]为你翘课的那一天
[01:08.00]花落的那一天
[01:14.00]教室的那一间
[01:20.00]我怎么看不见`

const SETTINGS_KEY = 'melodex.demo.settings'

/**
 * 演示用的音质档位表。
 *
 * 与 `crates/melodex-core/src/quality.rs` 保持一致——真实运行时这张表由后端
 * `platform_qualities` 命令下发，这里的副本只服务于浏览器预览。
 */
const DEMO_QUALITIES: PlatformQuality[] = [
  {
    source: 'netease',
    label: '网易云音乐',
    note: '登录后可用无损及以上；超清母带需要会员账号',
    items: [
      { code: 'jymaster', label: '超清母带', detail: 'FLAC 24bit/192kHz', level: 'master', bitrate: 3000, format: 'flac', vip: true },
      { code: 'hires', label: 'Hi-Res 无损', detail: 'FLAC 24bit/96kHz', level: 'hiRes', bitrate: 2000, format: 'flac', vip: true },
      { code: 'lossless', label: '无损音质', detail: 'FLAC 44.1kHz', level: 'lossless', bitrate: 1000, format: 'flac', vip: true },
      { code: 'exhigh', label: '极高音质', detail: 'MP3 320kbps', level: 'exquisite', bitrate: 320, format: 'mp3', vip: false },
      { code: 'higher', label: '较高音质', detail: 'MP3 192kbps', level: 'high', bitrate: 192, format: 'mp3', vip: false },
      { code: 'standard', label: '标准音质', detail: 'MP3 128kbps', level: 'standard', bitrate: 128, format: 'mp3', vip: false },
    ],
  },
  {
    source: 'qq',
    label: 'QQ 音乐',
    note: '登录后按账号权益取最高档：普通账号到 320k，绿钻到 SQ 无损，超级会员可试臻品母带/全景声',
    items: [
      { code: 'AIM0', label: '臻品母带', detail: 'FLAC 24bit/192kHz', level: 'master', bitrate: 3000, format: 'mflac', vip: true },
      { code: 'Q0M0', label: '臻品全景声', detail: 'FLAC 多声道', level: 'master', bitrate: 2800, format: 'mflac', vip: true },
      { code: 'RSM1', label: 'Hi-Res 无损', detail: 'FLAC 24bit/96kHz', level: 'hiRes', bitrate: 2000, format: 'mflac', vip: true },
      { code: 'F0M0', label: 'SQ 无损', detail: 'FLAC 44.1kHz', level: 'lossless', bitrate: 1000, format: 'mflac', vip: true },
      { code: 'M800', label: 'HQ 高品质', detail: 'MP3 320kbps', level: 'exquisite', bitrate: 320, format: 'mp3', vip: false },
      { code: 'M500', label: '标准音质', detail: 'MP3 128kbps', level: 'standard', bitrate: 128, format: 'mp3', vip: false },
    ],
  },
  {
    source: 'kuwo',
    label: '酷我音乐',
    note: '母带/全景声需要酷我会员账号；未登录多数歌曲只能取到 320k',
    items: [
      { code: '20900', label: '臻品母带', detail: 'FLAC 24bit/192kHz', level: 'master', bitrate: 3000, format: 'mflac', vip: true },
      { code: '20501', label: '臻品全景声 5.1', detail: 'FLAC 多声道', level: 'master', bitrate: 2800, format: 'mflac', vip: true },
      { code: '20201', label: '臻品全景声', detail: 'FLAC 多声道', level: 'hiRes', bitrate: 2500, format: 'mflac', vip: true },
      { code: '2000', label: '无损音质', detail: 'FLAC 44.1kHz', level: 'lossless', bitrate: 1000, format: 'flac', vip: true },
      { code: '320', label: '高品质', detail: 'MP3 320kbps', level: 'exquisite', bitrate: 320, format: 'mp3', vip: false },
      { code: '128', label: '标准音质', detail: 'MP3 128kbps', level: 'standard', bitrate: 128, format: 'mp3', vip: false },
      { code: '48', label: '流畅音质', detail: 'AAC 48kbps', level: 'standard', bitrate: 48, format: 'aac', vip: false },
    ],
  },
]

/** 演示模式的「我的歌单」：分平台各给两个，方便看分组效果 */
const DEMO_PLAYLISTS: PlaylistSummary[] = [
  { source: 'netease', id: 'demo-ne-1', name: '我喜欢的音乐', cover: null, trackCount: 128, creator: '我' },
  { source: 'netease', id: 'demo-ne-2', name: '华语流行精选', cover: null, trackCount: 46, creator: '我' },
  { source: 'netease', id: 'demo-ne-3', name: '深夜车载', cover: null, trackCount: 32, creator: '收藏的歌单' },
  { source: 'qq', id: 'demo-qq-1', name: '默认收藏', cover: null, trackCount: 87, creator: '我' },
  { source: 'qq', id: 'demo-qq-2', name: '周杰伦全收录', cover: null, trackCount: 63, creator: '我' },
]

/** 歌单详情用的曲目池（按平台筛出对应的 variant） */
const DEMO_DETAIL_TRACKS: Variant[] = TRACKS.flatMap((track) => track.variants)

const DEFAULT_SETTINGS: Settings = {
  downloadDir: 'Music/Melodex',
  quality: 'lossless',
  qualityOverrides: {},
  autoFallback: true,
  schema: 2,
  preferSource: null,
  namingTemplate: '{artist} - {title}',
  artistSeparator: ' / ',
  concurrency: 3,
  writeTags: true,
  embedCover: true,
  saveLyric: true,
  duplicatePolicy: 'keepBoth',
  theme: 'system',
  accent: 'violet',
  sources: [...ALL_SOURCES],
}

export function createDemoTransport(): Transport {
  const handlers = new Set<(event: EngineEvent) => void>()
  const tasks: DownloadTask[] = []
  const timers = new Map<string, ReturnType<typeof setInterval>>()
  let seeded = false

  function emit(event: EngineEvent) {
    handlers.forEach((handler) => handler(event))
  }

  function patch(id: string, change: Partial<DownloadTask>) {
    const task = tasks.find((item) => item.id === id)
    if (!task) return
    Object.assign(task, change)
    emit({ type: 'taskUpsert', payload: { ...task } })
  }

  function startTimer(id: string) {
    const timer = setInterval(() => {
      const task = tasks.find((item) => item.id === id)
      if (!task || task.state !== 'downloading') {
        clearInterval(timer)
        timers.delete(id)
        return
      }
      const total = task.total || 4_200_000
      const step = Math.min(total - task.downloaded, 90_000 + Math.random() * 220_000)
      const downloaded = Math.round(task.downloaded + step)
      if (downloaded >= total) {
        clearInterval(timer)
        timers.delete(id)
        patch(id, {
          downloaded: total,
          total,
          speedBps: 0,
          state: 'completed',
          path: `${demoDir}/${task.artists.join(', ')} - ${task.title}.${task.format}`,
          finishedAt: Math.round(Date.now() / 1000),
        })
        emit({
          type: 'notice',
          payload: { level: 'success', message: `《${task.title}》下载完成` },
        })
        return
      }
      patch(id, {
        downloaded,
        total,
        speedBps: Math.round(step * (1000 / 320)),
        state: downloaded > total * 0.82 ? 'tagging' : 'downloading',
      })
    }, 320)
    timers.set(id, timer)
  }

  const demoDir = 'Music/Melodex'

  /** 演示模式预置几条任务，方便直接看到各状态的样子。 */
  function seedTasks() {
    if (seeded) return
    seeded = true
    const now = Math.round(Date.now() / 1000)
    const make = (
      index: number,
      state: DownloadTask['state'],
      extra: Partial<DownloadTask> = {},
    ): DownloadTask => {
      const seed = CATALOG[index]
      return {
        id: `demo-seed-${index}`,
        title: seed.title,
        artists: seed.artists,
        album: seed.album,
        cover: null,
        source: seed.sources[0],
        sourceId: `${seed.sources[0]}-${index}-0`,
        bitrate: 320,
        format: 'mp3',
        durationMs: seed.durationMs,
        state,
        downloaded: Math.round((seed.durationMs / 1000) * 40_000),
        total: Math.round((seed.durationMs / 1000) * 40_000),
        speedBps: 0,
        path: `${demoDir}/${seed.artists.join(', ')} - ${seed.title}.mp3`,
        error: null,
        failReason: null,
        fallbacks: [],
        resolvedSource: null,
        quality: 'lossless',
        createdAt: now - index * 3600,
        finishedAt: now - index * 3600 + 60,
        ...extra,
      }
    }
    tasks.push(
      make(0, 'completed'),
      make(2, 'completed'),
      make(3, 'failed', {
        downloaded: 0,
        total: 0,
        path: null,
        source: 'qq',
        error: 'QQ 音乐：该音源必须登录后才提供下载链接，登录后重试即可',
        failReason: 'needLogin',
        finishedAt: now - 600,
      }),
      make(4, 'downloading', {
        downloaded: 1_820_000,
        total: 4_100_000,
        speedBps: 312_000,
        path: null,
        finishedAt: null,
      }),
    )
    setTimeout(() => startTimer('demo-seed-4'), 600)
  }

  return {
    mode: 'demo',

    async search(params: SearchParams) {
      await new Promise((resolve) => setTimeout(resolve, 260))
      const keyword = params.keyword.trim().toLowerCase()
      const enabled = params.sources.length ? params.sources : ALL_SOURCES
      const matched = TRACKS.filter((track) => {
        const hit =
          !keyword ||
          track.title.toLowerCase().includes(keyword) ||
          track.artists.some((artist) => artist.toLowerCase().includes(keyword))
        const hasSource = track.variants.some((variant) => enabled.includes(variant.source))
        return hit && hasSource
      }).map((track) => ({
        ...track,
        variants: track.variants.filter((variant) => enabled.includes(variant.source)),
      }))

      const page: AggregatedPage = {
        keyword: params.keyword,
        page: params.page,
        tracks: matched,
        reports: enabled.map((source) => ({
          source,
          ok: true,
          count: matched.filter((track) => track.variants.some((v) => v.source === source)).length,
          elapsedMs: 120 + Math.round(Math.random() * 200),
          error: null,
        })),
      }
      return page
    },

    async resolve(variant: Variant, quality?: QualityLevel): Promise<PlayUrl> {
      await new Promise((resolve) => setTimeout(resolve, 120))
      const tier =
        DEMO_QUALITIES.find((entry) => entry.source === variant.source)?.items.find(
          (item) => item.level === (quality ?? 'lossless'),
        ) ?? null
      return {
        url: `demo://${variant.source}/${variant.id}`,
        bitrate: tier?.bitrate ?? variant.bitrate,
        format: tier?.format ?? variant.format,
        size: variant.size,
      }
    },

    async platformQualities() {
      return DEMO_QUALITIES
    },

    async lyric(): Promise<Lyric | null> {
      await new Promise((resolve) => setTimeout(resolve, 80))
      return { lrc: DEMO_LYRIC, translation: null }
    },

    async myPlaylists(sources?: SourceId[]): Promise<PlaylistSummary[]> {
      await new Promise((resolve) => setTimeout(resolve, 120))
      const wanted: SourceId[] = sources?.length ? sources : [...ALL_SOURCES]
      return DEMO_PLAYLISTS.filter((item) => wanted.includes(item.source))
    },

    async playlist(ref: { source: SourceId; id: string }): Promise<PlaylistDetail | null> {
      await new Promise((resolve) => setTimeout(resolve, 140))
      const summary = DEMO_PLAYLISTS.find(
        (item) => item.source === ref.source && item.id === ref.id,
      )
      if (!summary) return null
      const items = DEMO_DETAIL_TRACKS.filter((variant) => variant.source === ref.source)
      return {
        source: ref.source,
        id: ref.id,
        name: summary.name,
        cover: summary.cover,
        total: items.length,
        items,
      }
    },

    async enqueue(inputs: EnqueueInput[]) {
      const ids: string[] = []
      inputs.forEach((input, index) => {
        const id = `demo-${Date.now()}-${index}`
        const task: DownloadTask = {
          id,
          title: input.title,
          artists: input.artists,
          album: input.album,
          cover: input.cover,
          source: input.source,
          sourceId: input.sourceId,
          bitrate: input.bitrate ?? 320,
          format: input.format ?? 'mp3',
          durationMs: input.durationMs,
          state: 'queued',
          downloaded: 0,
          total: Math.round((input.durationMs / 1000) * 40_000),
          speedBps: 0,
          path: null,
          error: null,
          failReason: null,
          fallbacks: input.fallbacks ?? [],
          resolvedSource: null,
          quality: input.quality ?? 'lossless',
          createdAt: Math.round(Date.now() / 1000),
          finishedAt: null,
        }
        tasks.unshift(task)
        ids.push(id)
        emit({ type: 'taskUpsert', payload: { ...task } })
        setTimeout(() => {
          const current = tasks.find((item) => item.id === id)
          if (!current || current.state !== 'queued') return
          patch(id, { state: 'downloading' })
          startTimer(id)
        }, 400 + index * 260)
      })
      return ids
    },

    async tasks() {
      seedTasks()
      return tasks.map((task) => ({ ...task }))
    },

    async pause(id: string) {
      const timer = timers.get(id)
      if (timer) {
        clearInterval(timer)
        timers.delete(id)
      }
      patch(id, { state: 'paused', speedBps: 0 })
    },

    async resume(id: string) {
      const task = tasks.find((item) => item.id === id)
      if (!task) return
      patch(id, { state: 'downloading', speedBps: 0 })
      startTimer(id)
    },

    async remove(id: string) {
      const timer = timers.get(id)
      if (timer) {
        clearInterval(timer)
        timers.delete(id)
      }
      const index = tasks.findIndex((task) => task.id === id)
      if (index >= 0) {
        tasks.splice(index, 1)
        emit({ type: 'taskRemoved', payload: id })
      }
    },

    async clearFinished() {
      let removed = 0
      for (let i = tasks.length - 1; i >= 0; i -= 1) {
        const state = tasks[i].state
        if (state === 'completed' || state === 'failed' || state === 'canceled') {
          tasks.splice(i, 1)
          removed += 1
        }
      }
      return removed
    },

    async getSettings() {
      try {
        const raw = localStorage.getItem(SETTINGS_KEY)
        if (raw) return { ...DEFAULT_SETTINGS, ...(JSON.parse(raw) as Settings) }
      } catch {
        /* 忽略损坏的本地配置 */
      }
      return { ...DEFAULT_SETTINGS }
    },

    async saveSettings(settings: Settings) {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
    },

    onEvent(handler) {
      handlers.add(handler)
      return () => handlers.delete(handler)
    },

    async defaultDownloadDir() {
      return demoDir
    },

    async pickDownloadDir() {
      /* 浏览器端无系统目录选择框 */
      return null
    },

    async reveal() {
      /* 浏览器端不支持打开文件位置 */
    },

    async checkUpdate() {
      // 演示一份"有新版本"的清单，方便走查更新弹窗；真实检查在 Tauri 端由 Rust 完成
      await new Promise((resolve) => setTimeout(resolve, 700))
      return {
        current: '1.1.2',
        latest: 'v0.6.1',
        available: true,
        notes:
          '新增\n· 酷狗音源适配\n· 下载队列并发数热调整\n\n修复\n· 移动端深色主题下封面边框对比度\n· 断点续传在代理环境下的 416 处理',
        pageUrl: 'https://github.com/LunaShiori/melodex/releases/latest',
        publishedAt: '2026-10-01T08:00:00Z',
        assets: [
          {
            name: 'Melodex_0.6.1_x64-setup.exe',
            size: 4_612_345,
            url: 'https://github.com/LunaShiori/melodex/releases/latest',
            kind: 'nsis',
          },
          {
            name: 'Melodex_0.6.1_x64_en-US.msi',
            size: 5_180_224,
            url: 'https://github.com/LunaShiori/melodex/releases/latest',
            kind: 'msi',
          },
          {
            name: 'Melodex_0.6.1_universal.apk',
            size: 9_812_006,
            url: 'https://github.com/LunaShiori/melodex/releases/latest',
            kind: 'apk',
          },
        ],
      }
    },

    updateDownload(info, assetName) {
      const asset = assetName
        ? info.assets.find((item) => item.name === assetName)
        : info.assets[0]
      const total = asset?.size ?? 4_000_000
      // 模拟分片下载：每 60ms 推进一格，让进度条与真实网络观感接近
      return new Promise<string>((resolve) => {
        let done = 0
        const step = Math.max(1, Math.round(total / 36))
        const timer = setInterval(() => {
          done = Math.min(total, done + step)
          for (const listener of updateListeners) {
            listener({ downloaded: done, total })
          }
          if (done >= total) {
            clearInterval(timer)
            resolve(`/demo/updates/${asset?.name ?? 'Melodex-setup.exe'}`)
          }
        }, 60)
      })
    },

    async updateInstall() {
      throw new Error('演示模式没有可执行的安装器，请真机运行后再试')
    },

    onUpdateProgress(handler) {
      updateListeners.add(handler)
      return () => {
        updateListeners.delete(handler)
      }
    },

    async openUrl(url: string) {
      window.open(url, '_blank', 'noopener')
    },

    localUrl() {
      return ''
    },

    /* ---- 账号：浏览器里用内存模拟，方便走查扫码流程 ---- */

    async vaultStatus() {
      return {
        exists: true,
        pinProtected: false,
        unlocked: true,
        accounts: demoAccounts(),
        minPinLen: 6,
      } satisfies VaultStatus
    },

    async vaultUnlock() {
      return this.vaultStatus()
    },

    async vaultSetPin(pin: string | null) {
      const status = await this.vaultStatus()
      return { ...status, pinProtected: Boolean(pin) }
    },

    async loginQrStart(source: SourceId) {
      demoQrTicks = 0
      const key = `demo-${Date.now()}`
      return {
        source,
        key,
        qrUrl: `https://music.163.com/login?codekey=${key}`,
        expiresIn: 180,
        qrSvg: fakeQrSvg(),
        qrImage: null,
      } satisfies QrTicket
    },

    async loginQrPoll(source: SourceId) {
      demoQrTicks += 1
      if (demoQrTicks <= 2) {
        return { status: 'waiting', cookie: null, nickname: null, message: '等待扫码' } satisfies QrPoll
      }
      if (demoQrTicks === 3) {
        return {
          status: 'scanned',
          cookie: null,
          nickname: null,
          message: '扫描成功，请在手机上确认',
        } satisfies QrPoll
      }
      const account: AccountInfo = {
        source,
        nickname: '演示账号',
        savedAt: Math.floor(Date.now() / 1000),
        keys: ['MUSIC_U', '__csrf'],
      }
      demoAccounts_.set(source, account)
      return {
        status: 'confirmed',
        cookie: 'MUSIC_U=demo',
        nickname: account.nickname,
        message: '登录成功',
      } satisfies QrPoll
    },

    async saveAccount(source: SourceId, _cookie: string, nickname?: string | null) {
      const account: AccountInfo = {
        source,
        nickname: nickname || (source === 'qq' ? 'QQ 演示账号' : '演示账号'),
        savedAt: Math.floor(Date.now() / 1000),
        keys: source === 'qq' ? ['uin', 'qm_keystore'] : ['MUSIC_U'],
      }
      demoAccounts_.set(source, account)
      return account
    },

    async clearAccount(source: SourceId) {
      demoAccounts_.delete(source)
      return this.vaultStatus()
    },

    /* ---- 内置网页登录：浏览器里假装用户已经在网页上登录好了 ---- */

    loginWebSupported() {
      // 浏览器预览没有内置 WebView，但返回 true 并模拟成功，
      // 这样等待态、成功态都能在设计走查里看到
      return Promise.resolve(true)
    },

    async loginWebStart(source: SourceId) {
      window.setTimeout(() => {
        const account: AccountInfo = {
          source,
          nickname: source === 'qq' ? 'QQ 演示账号' : '演示账号',
          savedAt: Math.floor(Date.now() / 1000),
          keys: source === 'qq' ? ['uin', 'qm_keyst'] : ['MUSIC_U'],
        }
        demoAccounts_.set(source, account)
        for (const handler of loginWebHandlers) handler({ type: 'done', account })
      }, 1600)
    },

    onLoginWeb(handler: (event: LoginWebEvent) => void) {
      loginWebHandlers.add(handler)
      return () => {
        loginWebHandlers.delete(handler)
      }
    },
  }
}
