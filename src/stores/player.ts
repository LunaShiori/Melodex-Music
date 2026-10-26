import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { activeLineIndex, mergeTranslation, parseLrc, type LrcLine } from '@/lib/lrc'
import { transport } from '@/lib/transport'
import type { FailReason, PlayUrl, SourceId, Track, Variant } from '@/lib/types'
import { bestVariant } from '@/lib/track'

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
  const lyricOpen = ref(false)
  /** 手机端的全屏播放页是否展开 */
  const expanded = ref(false)
  /** 正在取链接 / 起播（按钮转圈） */
  const loading = ref(false)
  const error = ref<PlayerError | null>(null)
  /** 本次播放是否已经退到代理地址，避免同一个错误反复重试 */
  let triedProxy = false

  const audio = typeof Audio !== 'undefined' ? new Audio() : null
  const isDemo = transport.mode === 'demo'
  let ticker: ReturnType<typeof setInterval> | null = null

  const progress = computed(() => (duration.value ? position.value / duration.value : 0))
  const activeLyricIndex = computed(() => activeLineIndex(lyricLines.value, position.value))

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
      if (code === 2) {
        error.value = mediaError('linkRejected', '音频请求失败或被服务器中断', target)
      } else if (code === 3 || code === 4) {
        error.value = mediaError('qualityMissing', '当前播放器无法解码这个音频格式', target)
      } else {
        error.value = mediaError('unknown', '音频加载失败，可能链接已过期', target)
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

  async function loadLyric(target: Variant) {
    lyricLines.value = []
    try {
      const result = await transport.lyric({ source: target.source, id: target.id })
      if (!result) return
      lyricLines.value = mergeTranslation(parseLrc(result.lrc), result.translation)
    } catch {
      lyricLines.value = []
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
      error.value = null
      return true
    } catch (err) {
      // 源被 WebView 拒绝时，换补了 Referer 的代理地址再试一次
      const detail = err instanceof Error ? err.message : String(err)
      if (!triedProxy && source.proxyUrl && source.proxyUrl !== source.url) {
        triedProxy = true
        return attach({ ...source, url: source.proxyUrl }, target)
      }
      error.value = mediaError('linkRejected', detail, target.source)
      return false
    }
  }

  async function playTrack(track: Track, chosen?: Variant) {
    const target = chosen ?? bestVariant(track)
    current.value = track
    variant.value = target
    duration.value = track.durationMs || 0
    position.value = 0
    error.value = null
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
      error.value = toPlayerError(err, target.source)
      playing.value = false
    } finally {
      loading.value = false
    }
  }

  /** 播放本地已下载文件（桌面端走 asset 协议）。 */
  async function playLocal(track: Track, src: string) {
    current.value = track
    variant.value = track.variants[0] ?? null
    duration.value = track.durationMs || 0
    position.value = 0
    error.value = null
    loading.value = true
    lyricLines.value = []
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
      error.value = mediaError('unknown', err instanceof Error ? err.message : String(err), 'netease')
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

  function toggle() {
    if (!current.value) return
    if (isDemo) {
      playing.value = !playing.value
      startTicker()
      return
    }
    if (!audio) return
    // 上一次失败过（没有 src，或 src 已被 CDN 拒）就重新取一遍链接
    if (!audio.src || error.value) {
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
    lyricOpen,
    expanded,
    loading,
    error,
    progress,
    activeLyricIndex,
    isDemo,
    playTrack,
    playLocal,
    playList,
    next,
    prev,
    toggle,
    seek,
    setVolume,
    isCurrent,
  }
})
