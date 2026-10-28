import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { pushRecent } from '@/lib/history'
import { activeLineIndex, mergeTranslation, parseLrc, type LrcLine } from '@/lib/lrc'
import { transport } from '@/lib/transport'
import type { FailReason, PlayUrl, SourceId, Track, Variant } from '@/lib/types'
import { bestVariant } from '@/lib/track'
import { useLyricStore } from './lyric'

/** 播放失败的完整说明。内核给得出来就用内核的，给不出来就按媒体错误码兜底。 */
export interface PlayerError {
  reason: FailReason
  label: string
  message: string
  hint: string
  source: SourceId
}

/** 把内核抛出的结构体识别成 {@link PlayerError}；不是的话退回字符串文案。 */
export function toPlayerError(err: unknown, source: SourceId): PlayerError {
  if (err && typeof err === 'object' && 'reason' in err && 'message' in err) {
    const raw = err as Partial<PlayerError>
    return {
      reason: (raw.reason as FailReason) ?? 'unknown',
      label: raw.label ?? '无法播放',
      message: raw.message ?? '音源没有返回可用链接',
      hint: raw.hint ?? '稍后重试或换一个音源',
      source: raw.source ?? source,
    }
  }
  const message = err instanceof Error ? err.message : String(err)
  return {
    reason: 'unknown',
    label: '无法播放',
    message,
    hint: '稍后重试或换一个音源',
    source,
  }
}

/** `<audio>` 的 MediaError 转成人话。 */
function mediaError(reason: FailReason, detail: string, source: SourceId): PlayerError {
  return {
    reason,
    label: reason === 'linkRejected' ? '链接已失效' : '无法播放',
    message: detail,
    hint:
      reason === 'linkRejected'
        ? '音源给的地址被它自己的服务器拒绝了，重新点一次播放会重新取链接'
        : '当前播放器可能解不了这个格式，试试换一个音源',
    source,
  }
}

/**
 * 全局播放器。演示模式没有真实音频，用计时器推进进度，
 * 保证界面（进度条、歌词高亮）与正式端行为一致。
 */
export const usePlayerStore = defineStore('player', () => {
  const queue = ref<Track[]>([])
  const index = ref(-1)
  const current = ref<Track | null>(null)
  const variant = ref<Variant | null>(null)
  const playing = ref(false)
  const position = ref(0)
  const duration = ref(0)
  const volume = ref(0.8)
  const lyricLines = ref<LrcLine[]>([])
  /** 歌词详情页（手机的全屏播放页 / 桌面的歌词详情）是否打开——两端共用一个入口 */
  const detailOpen = ref(false)
  /** 正在取链接 / 起播（按钮转圈） */
  const loading = ref(false)
  const error = ref<PlayerError | null>(null)
  /** 本次播放是否已经退到代理地址，避免同一个错误反复重试 */
  let triedProxy = false
  /** 歌词请求序号，用于丢弃「切歌之后才回来的」过期结果 */
  let lyricToken = 0

  const audio = typeof Audio !== 'undefined' ? new Audio() : null
  const isDemo = transport.mode === 'demo'
  let ticker: ReturnType<typeof setInterval> | null = null

  const lyric = useLyricStore()

  const progress = computed(() => (duration.value ? position.value / duration.value : 0))
  /**
   * 当前唱到第几行。`offsetMs` 是用户手动对齐的偏移：
   * 正值表示「歌词整体出现得更晚」，所以拿播放位置减去它再查表。
   */
  const activeLyricIndex = computed(() =>
    activeLineIndex(lyricLines.value, position.value - lyric.offsetMs),
  )
  /** 当前这一行的文本，给播放条上的跑马灯用 */
  const currentLyric = computed(() => {
    const index = activeLyricIndex.value
    if (index < 0) return ''
    return lyricLines.value[index]?.text?.trim() ?? ''
  })

  /**
   * 失败提示不能「一有错误就弹」。
   *
   * 手机上切歌/换源的一瞬间，WebView 会先抛一次短命的 MEDIA_ERR（解码器还没就绪），
   * 几十毫秒后自己就好了。旧代码直接把 `error` 写进去，界面上就是
   * 「点播放先闪一个红色小窗，然后才正常播」。现在分成两级：
   * - `armError`：观察到错误，先压 450ms，期间恢复正常就当没发生
   * - `showError`：调用方已经确认这是最终失败（重试也用完了），立刻展示
   */
  const broken = ref(false)
  let pendingError: PlayerError | null = null
  let errorTimer: ReturnType<typeof setTimeout> | null = null

  function cancelErrorTimer() {
    if (errorTimer) {
      clearTimeout(errorTimer)
      errorTimer = null
    }
  }

  function showError(next: PlayerError) {
    cancelErrorTimer()
    pendingError = null
    error.value = next
    broken.value = true
  }

  function armError(next: PlayerError) {
    pendingError = next
    if (errorTimer) return
    errorTimer = setTimeout(() => {
      errorTimer = null
      if (pendingError) {
        error.value = pendingError
        pendingError = null
        broken.value = true
      }
    }, 450)
  }

  function clearError() {
    cancelErrorTimer()
    pendingError = null
    error.value = null
    broken.value = false
  }

  if (audio) {
    audio.volume = volume.value
    audio.preload = 'auto'
    audio.addEventListener('timeupdate', () => {
      position.value = audio.currentTime * 1000
    })
    audio.addEventListener('loadedmetadata', () => {
      if (Number.isFinite(audio.duration) && audio.duration > 0) {
        duration.value = audio.duration * 1000
      }
    })
    audio.addEventListener('playing', () => {
      playing.value = true
      loading.value = false
      // 已经开始出声说明刚才那个错误是短命的，撤回提示
      clearError()
    })
    audio.addEventListener('canplay', () => {
      // 解码器就绪 = 刚才那个错误已经自愈，把还没弹出来的提示直接撤掉
      if (!broken.value) {
        cancelErrorTimer()
        pendingError = null
      }
    })
    audio.addEventListener('waiting', () => {
      if (playing.value) loading.value = true
    })
    audio.addEventListener('pause', () => {
      playing.value = false
    })
    audio.addEventListener('ended', () => {
      void next()
    })
    audio.addEventListener('error', () => {
      if (isDemo) return
      const code = audio.error?.code ?? 0
      // 1 = ABORTED：切歌或拖动引起的中断，不是真错误
      if (code === 1) return
      const target = variant.value?.source ?? 'netease'
      playing.value = false
      loading.value = false
      // 先「观望」再展示：换源瞬间的假错误不能弹窗
      if (code === 2) {
        armError(mediaError('linkRejected', '音频请求失败或被服务器中断', target))
      } else if (code === 3 || code === 4) {
        armError(mediaError('qualityMissing', '当前播放器无法解码这个音频格式', target))
      } else {
        armError(mediaError('unknown', '音频加载失败，可能链接已过期', target))
      }
    })
  }

  function startTicker() {
    if (ticker) return
    ticker = setInterval(() => {
      if (!isDemo || !playing.value) return
      position.value = Math.min(duration.value, position.value + 250)
      if (duration.value && position.value >= duration.value) void next()
    }, 250)
  }

  /**
   * 拉当前这首歌的歌词。
   *
   * 用递增 token 做「后发先至」保护：快速切歌时先发出的请求可能后回来，
   * 不加保护就会把上一首的歌词盖到当前这首上——表现就是「歌词跟歌对不上」。
   */
  async function loadLyric(target: Variant) {
    const token = ++lyricToken
    lyricLines.value = []
    try {
      const result = await transport.lyric({ source: target.source, id: target.id })
      if (token !== lyricToken) return
      if (!result) return
      lyricLines.value = mergeTranslation(parseLrc(result.lrc), result.translation)
    } catch {
      if (token === lyricToken) lyricLines.value = []
    }
  }

  /** 把一条已解析的地址塞进 `<audio>` 并起播。 */
  async function attach(source: PlayUrl, target: Variant): Promise<boolean> {
    if (!audio) return false
    audio.src = source.url
    audio.volume = volume.value
    try {
      await audio.play()
      playing.value = true
      clearError()
      return true
    } catch (err) {
      // 源被 WebView 拒绝时，换补了 Referer 的代理地址再试一次
      const detail = err instanceof Error ? err.message : String(err)
      if (!triedProxy && source.proxyUrl && source.proxyUrl !== source.url) {
        triedProxy = true
        return attach({ ...source, url: source.proxyUrl }, target)
      }
      showError(mediaError('linkRejected', detail, target.source))
      return false
    }
  }

  async function playTrack(track: Track, chosen?: Variant) {
    const target = chosen ?? bestVariant(track)
    current.value = track
    pushRecent(track)
    variant.value = target
    duration.value = track.durationMs || 0
    position.value = 0
    clearError()
    loading.value = true
    triedProxy = false
    void loadLyric(target)

    if (isDemo) {
      loading.value = false
      playing.value = true
      startTicker()
      return
    }
    if (!audio) return
    try {
      // 试听只要够快：固定取「极高音质」这一档，不跟随下载用的无损设置，
      // 否则每次点播放都要等一个几十兆的 FLAC 起播。
      // 取不到时内核抛结构化原因，界面直接照抄给用户看。
      const resolved = await transport.resolve(target, 'exquisite')
      await attach(resolved, target)
    } catch (err) {
      showError(toPlayerError(err, target.source))
      playing.value = false
    } finally {
      loading.value = false
    }
  }

  /** 播放本地已下载文件（桌面端走 asset 协议）。 */
  async function playLocal(track: Track, src: string) {
    current.value = track
    pushRecent(track)
    variant.value = track.variants[0] ?? null
    duration.value = track.durationMs || 0
    position.value = 0
    clearError()
    loading.value = true
    // 本地文件本身不带歌词，但仍然知道它来自哪个平台的哪首歌，
    // 所以照样去源站拉一次——否则从曲库起播时歌词区会一直是上一首的残留。
    if (variant.value) {
      void loadLyric(variant.value)
    } else {
      lyricLines.value = []
      lyricToken += 1
    }
    if (isDemo || !src) {
      loading.value = false
      playing.value = true
      startTicker()
      return
    }
    if (!audio) return
    audio.src = src
    audio.volume = volume.value
    try {
      await audio.play()
      playing.value = true
    } catch (err) {
      showError(mediaError('unknown', err instanceof Error ? err.message : String(err), 'netease'))
    } finally {
      loading.value = false
    }
  }

  async function playList(tracks: Track[], startIndex = 0) {
    queue.value = tracks
    index.value = startIndex
    const track = tracks[startIndex]
    if (track) await playTrack(track)
  }

  async function next() {
    if (!queue.value.length) {
      playing.value = false
      return
    }
    index.value = (index.value + 1) % queue.value.length
    const track = queue.value[index.value]
    if (track) await playTrack(track)
  }

  async function prev() {
    if (!queue.value.length) return
    index.value = (index.value - 1 + queue.value.length) % queue.value.length
    const track = queue.value[index.value]
    if (track) await playTrack(track)
  }

  /* ---------- 播放队列面板 ---------- */

  const queueOpen = ref(false)

  function toggleQueue() {
    queueOpen.value = !queueOpen.value
  }

  function closeQueue() {
    queueOpen.value = false
  }

  /** 点队列里的某一首直接播 */
  async function jumpTo(position: number) {
    const track = queue.value[position]
    if (!track) return
    index.value = position
    await playTrack(track)
  }

  /**
   * 从队列里移除一首。
   *
   * 删的是正在播的那首时顺位接着播下一首（列表因此变短，位置要夹一下）；
   * 删掉队列里的最后一首时停在这里，不自动停播也不报错。
   */
  async function removeAt(position: number) {
    if (position < 0 || position >= queue.value.length) return
    const playingIndex = index.value
    queue.value.splice(position, 1)
    if (!queue.value.length) {
      index.value = -1
      return
    }
    if (position === playingIndex) {
      index.value = Math.min(playingIndex, queue.value.length - 1)
      const track = queue.value[index.value]
      if (track) await playTrack(track)
      return
    }
    if (position < playingIndex) index.value = playingIndex - 1
  }

  /** 拖动排序：把 from 挪到 to，正在播的那首跟着它自己的位置走 */
  function move(from: number, to: number) {
    const list = queue.value
    if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return
    const [item] = list.splice(from, 1)
    if (!item) return
    list.splice(to, 0, item)
    if (index.value === from) {
      index.value = to
    } else if (from < index.value && to >= index.value) {
      index.value -= 1
    } else if (from > index.value && to <= index.value) {
      index.value += 1
    }
  }

  function toggle() {
    if (!current.value) return
    if (isDemo) {
      playing.value = !playing.value
      startTicker()
      return
    }
    if (!audio) return
    // 上一次失败过（没有 src，或 src 已被 CDN 拒）就重新取一遍链接
    if (!audio.src || broken.value) {
      void playTrack(current.value, variant.value ?? undefined)
      return
    }
    if (audio.paused) {
      void audio
        .play()
        .then(() => {
          playing.value = true
        })
        .catch(() => {
          if (current.value) void playTrack(current.value, variant.value ?? undefined)
        })
    } else {
      audio.pause()
      playing.value = false
    }
  }

  function seek(ms: number) {
    position.value = Math.max(0, Math.min(duration.value || ms, ms))
    // 元数据还没加载时 duration 是 Infinity，此时不能写 currentTime
    if (!isDemo && audio && Number.isFinite(audio.duration)) {
      audio.currentTime = position.value / 1000
    }
  }

  function setVolume(next: number) {
    volume.value = Math.max(0, Math.min(1, next))
    if (audio) audio.volume = volume.value
  }

  function isCurrent(track: Track) {
    return current.value?.fingerprint === track.fingerprint
  }

  return {
    queue,
    index,
    current,
    variant,
    playing,
    position,
    duration,
    volume,
    lyricLines,
    detailOpen,
    loading,
    error,
    progress,
    activeLyricIndex,
    currentLyric,
    isDemo,
    playTrack,
    playLocal,
    playList,
    next,
    prev,
    queueOpen,
    toggleQueue,
    closeQueue,
    jumpTo,
    removeAt,
    move,
    toggle,
    seek,
    setVolume,
    isCurrent,
  }
})
