/**
 * 系统媒体控制。
 *
 * 国内手机的「灵动岛 / 锁屏音乐卡 / 状态栏媒体通知」读的都是系统媒体会话，
 * 也就是浏览器的 MediaSession API —— WebView 里只要把元数据和播放动作挂上去，
 * 系统和各家的岛就会自动显示，不需要任何厂商 SDK。
 *
 * 这里做成薄封装：不支持的环境（老 WebView、桌面浏览器）全部静默跳过，
 * 调用方不用到处写 `if ('mediaSession' in navigator)`。
 */

export interface NowPlaying {
  title: string
  artist: string
  album: string
  cover: string | null
  durationMs: number
}

/** 播放动作的回调集合；没实现的动作不挂，系统会自动隐藏对应按钮 */
export interface MediaActions {
  play?: () => void
  pause?: () => void
  next?: () => void
  prev?: () => void
  stop?: () => void
  seekTo?: (positionSec: number) => void
  seekBy?: (deltaSec: number) => void
}

const session = typeof navigator !== 'undefined' ? navigator.mediaSession : undefined
export const supportsMediaSession = Boolean(session)

/** 封面统一给 512，太小系统会拉伸、太大又白占内存 */
function artwork(cover: string | null): MediaImage[] | undefined {
  if (!cover) return undefined
  return [{ src: cover, sizes: '512x512', type: cover.startsWith('data:') ? 'image/jpeg' : undefined }]
}

export function setNowPlaying(info: NowPlaying | null) {
  if (!session) return
  if (!info) {
    session.metadata = null
    return
  }
  if (typeof MediaMetadata === 'undefined') return
  session.metadata = new MediaMetadata({
    title: info.title,
    artist: info.artist,
    album: info.album,
    artwork: artwork(info.cover),
  })
}

export function setPlaybackState(state: 'playing' | 'paused' | 'none') {
  if (!session) return
  session.playbackState = state
}

/**
 * 上报进度。系统的进度条和「拖动跳转」依赖这个，不报的话锁屏上就是一条死线。
 * 调用方要自己节流（每播放一秒报一次足够）。
 */
export function setPosition(positionMs: number, durationMs: number) {
  if (!session?.setPositionState) return
  if (!Number.isFinite(durationMs) || durationMs <= 0) return
  const duration = durationMs / 1000
  const position = Math.min(Math.max(positionMs / 1000, 0), duration)
  try {
    session.setPositionState({ duration, position, playbackRate: 1 })
  } catch {
    // 部分实现会拒绝 position > duration 这类边界值，忽略即可
  }
}

export function bindActions(actions: MediaActions) {
  if (!session) return () => {}
  const handles: [MediaSessionAction, MediaSessionActionHandler | null][] = [
    ['play', actions.play ? () => actions.play?.() : null],
    ['pause', actions.pause ? () => actions.pause?.() : null],
    ['nexttrack', actions.next ? () => actions.next?.() : null],
    ['previoustrack', actions.prev ? () => actions.prev?.() : null],
    ['stop', actions.stop ? () => actions.stop?.() : null],
    ['seekto', actions.seekTo ? (details) => actions.seekTo?.(details.seekTime ?? 0) : null],
    [
      'seekbackward',
      actions.seekBy ? (details) => actions.seekBy?.(-(details.seekOffset ?? 10)) : null,
    ],
    [
      'seekforward',
      actions.seekBy ? (details) => actions.seekBy?.(details.seekOffset ?? 10) : null,
    ],
  ]
  for (const [action, handler] of handles) {
    try {
      session.setActionHandler(action, handler)
    } catch {
      // 未实现的 action 会抛错，逐个吞掉不影响其它按钮
    }
  }
  return function unbind() {
    for (const [action] of handles) {
      try {
        session?.setActionHandler(action, null)
      } catch {
        /* 同上 */
      }
    }
  }
}
