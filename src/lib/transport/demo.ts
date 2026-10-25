import type {
  AggregatedPage,
  DownloadTask,
  EngineEvent,
  EnqueueInput,
  Lyric,
  PlayUrl,
  Settings,
  SourceId,
  Variant,
} from '../types'
import { ALL_SOURCES } from '../types'
import type { SearchParams, Transport } from './types'

/**
 * 演示传输层：浏览器里没有 Rust 核心，这里用内置曲库 + 模拟进度
 * 把整套交互跑通，方便 UI 走查与截图。正式功能全部由 Tauri 端提供。
 */

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

const DEFAULT_SETTINGS: Settings = {
  downloadDir: 'Music/Melodex',
  bitrate: 320,
  fallbackBitrates: [320, 192, 128],
  preferSource: null,
  namingTemplate: '{artist} - {title}',
  artistSeparator: ' / ',
  concurrency: 3,
  writeTags: true,
  embedCover: true,
  saveLyric: true,
  duplicatePolicy: 'keepBoth',
  theme: 'system',
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
        createdAt: now - index * 3600,
        finishedAt: now - index * 3600 + 60,
        ...extra,
      }
    }
    tasks.push(
      make(0, 'completed'),
      make(2, 'completed'),
      make(3, 'failed', {
        downloaded: 1_240_000,
        total: 4_600_000,
        path: null,
        error: '该歌曲没有可用的音频链接',
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

    async resolve(variant: Variant, bitrate = 320): Promise<PlayUrl> {
      await new Promise((resolve) => setTimeout(resolve, 120))
      return {
        url: `demo://${variant.source}/${variant.id}`,
        bitrate,
        format: variant.format,
        size: variant.size,
      }
    },

    async lyric(): Promise<Lyric | null> {
      await new Promise((resolve) => setTimeout(resolve, 80))
      return { lrc: DEMO_LYRIC, translation: null }
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

    async reveal() {
      /* 浏览器端不支持打开文件位置 */
    },

    async checkUpdate() {
      // 演示一份"有新版本"的清单，方便走查更新弹窗；真实检查在 Tauri 端由 Rust 完成
      await new Promise((resolve) => setTimeout(resolve, 700))
      return {
        current: '0.1.0',
        latest: 'v0.2.0',
        available: true,
        notes:
          '新增\n· 酷狗音源适配\n· 下载队列并发数热调整\n\n修复\n· 移动端深色主题下封面边框对比度\n· 断点续传在代理环境下的 416 处理',
        pageUrl: 'https://github.com/melodex-app/melodex/releases/latest',
        publishedAt: '2026-10-01T08:00:00Z',
        assets: [
          {
            name: 'Melodex_0.2.0_x64-setup.exe',
            size: 4_612_345,
            url: 'https://github.com/melodex-app/melodex/releases/latest',
            kind: 'nsis',
          },
          {
            name: 'Melodex_0.2.0_x64_en-US.msi',
            size: 5_180_224,
            url: 'https://github.com/melodex-app/melodex/releases/latest',
            kind: 'msi',
          },
          {
            name: 'Melodex_0.2.0_universal.apk',
            size: 9_812_006,
            url: 'https://github.com/melodex-app/melodex/releases/latest',
            kind: 'apk',
          },
        ],
      }
    },

    async openUrl(url: string) {
      window.open(url, '_blank', 'noopener')
    },

    localUrl() {
      return ''
    },
  }
}
