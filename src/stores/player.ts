import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { activeLineIndex, mergeTranslation, parseLrc, type LrcLine } from '@/lib/lrc'
import { transport } from '@/lib/transport'
import type { Track, Variant } from '@/lib/types'
import { bestVariant } from '@/lib/track'

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
  const error = ref<string | null>(null)

  const audio = typeof Audio !== 'undefined' ? new Audio() : null
  const isDemo = transport.mode === 'demo'
  let ticker: ReturnType<typeof setInterval> | null = null

  const progress = computed(() => (duration.value ? position.value / duration.value : 0))
  const activeLyricIndex = computed(() => activeLineIndex(lyricLines.value, position.value))

  if (audio) {
    audio.volume = volume.value
    audio.addEventListener('timeupdate', () => {
      position.value = audio.currentTime * 1000
    })
    audio.addEventListener('loadedmetadata', () => {
      if (Number.isFinite(audio.duration) && audio.duration > 0) {
        duration.value = audio.duration * 1000
      }
    })
    audio.addEventListener('ended', () => {
      void next()
    })
    audio.addEventListener('error', () => {
      if (!isDemo) {
        error.value = '音频加载失败，可能链接已过期'
        playing.value = false
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

  async function playTrack(track: Track, chosen?: Variant) {
    const target = chosen ?? bestVariant(track)
    current.value = track
    variant.value = target
    duration.value = track.durationMs || 0
    position.value = 0
    error.value = null
    void loadLyric(target)

    if (isDemo) {
      playing.value = true
      startTicker()
      return
    }
    if (!audio) return
    try {
      // 试听只要够快：固定取 320k 这一档，不跟随下载用的无损设置，
      // 否则每次点播放都要等一个几十兆的 FLAC 起播
      const resolved = await transport.resolve(target, 'exquisite')
      audio.src = resolved.url
      audio.volume = volume.value
      await audio.play()
      playing.value = true
      if (!duration.value && resolved.size) duration.value = track.durationMs
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
      playing.value = false
    }
  }

  /** 播放本地已下载文件（桌面端走 asset 协议）。 */
  async function playLocal(track: Track, src: string) {
    current.value = track
    variant.value = track.variants[0] ?? null
    duration.value = track.durationMs || 0
    position.value = 0
    error.value = null
    lyricLines.value = []
    if (isDemo || !src) {
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
      error.value = err instanceof Error ? err.message : String(err)
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
    if (audio.paused) {
      void audio.play().then(() => {
        playing.value = true
      })
    } else {
      audio.pause()
      playing.value = false
    }
  }

  function seek(ms: number) {
    position.value = Math.max(0, Math.min(duration.value || 0, ms))
    if (!isDemo && audio) audio.currentTime = position.value / 1000
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
