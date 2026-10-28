/**
 * 搜索历史与最近播放。
 *
 * 两样都只存 localStorage、不进设置文件：纯本地记录，清掉不影响任何功能。
 * 播放记录存完整 `Track`（含各音源 Variant），所以点一条就能直接重播，
 * 不用再搜一遍——音源链接是每次播放时重新取的，过期不影响。
 */

import { ref } from 'vue'

import type { Track } from './types'

const KEY = 'melodex.history.v1'
const MAX_KEYWORDS = 10
const MAX_RECENT = 20

/** 最近搜过的词，新的在前 */
export const keywords = ref<string[]>([])
/** 最近播放的曲目，新的在前 */
export const recent = ref<Track[]>([])

interface Snapshot {
  keywords: string[]
  recent: Track[]
}

function isTrack(value: unknown): value is Track {
  if (!value || typeof value !== 'object') return false
  const track = value as Partial<Track>
  return (
    typeof track.fingerprint === 'string' &&
    typeof track.title === 'string' &&
    Array.isArray(track.variants) &&
    track.variants.length > 0
  )
}

function read(): Snapshot {
  const empty: Snapshot = { keywords: [], recent: [] }
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return empty
    const parsed = JSON.parse(raw) as Partial<Snapshot>
    return {
      keywords: Array.isArray(parsed.keywords)
        ? parsed.keywords.filter((word): word is string => typeof word === 'string').slice(0, MAX_KEYWORDS)
        : [],
      recent: Array.isArray(parsed.recent)
        ? parsed.recent.filter(isTrack).slice(0, MAX_RECENT)
        : [],
    }
  } catch {
    /* 隐私模式或数据损坏：当作没有历史，不影响搜索 */
    return empty
  }
}

function write() {
  try {
    localStorage.setItem(
      KEY,
      JSON.stringify({ keywords: keywords.value, recent: recent.value.slice(0, MAX_RECENT) }),
    )
  } catch {
    /* 写不进去就只在本次会话里生效 */
  }
}

let loaded = false

/** 启动时读一次；重复调用无副作用 */
export function loadHistory() {
  if (loaded) return
  loaded = true
  const snapshot = read()
  keywords.value = snapshot.keywords
  recent.value = snapshot.recent
}

/** 记一次搜索。空白和重复的会被并到最前面，不重复占坑。 */
export function pushKeyword(word: string) {
  const next = word.trim()
  if (!next) return
  keywords.value = [next, ...keywords.value.filter((item) => item !== next)].slice(0, MAX_KEYWORDS)
  write()
}

/** 记一次播放。同一首重复播只留最新一条。 */
export function pushRecent(track: Track) {
  recent.value = [
    track,
    ...recent.value.filter((item) => item.fingerprint !== track.fingerprint),
  ].slice(0, MAX_RECENT)
  write()
}

export function clearKeywords() {
  keywords.value = []
  write()
}

export function clearRecent() {
  recent.value = []
  write()
}
