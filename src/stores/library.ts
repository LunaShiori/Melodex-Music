import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import type { DownloadTask } from '@/lib/types'

/**
 * 本地曲库。
 *
 * 与「下载记录」分开存放：下载记录只描述本次会话里的任务进度，清空记录不该影响
 * 已经落盘的音乐；曲库保存的是下载完成的快照，持久化在 localStorage，只有用户
 * 主动「移出列表」或「清空曲库」才会减少。
 */
const STORE_KEY = 'melodex.library.v1'
const MAX_ENTRIES = 2000

export interface LibraryEntry {
  id: string
  title: string
  artists: string[]
  album: string
  cover: string | null
  source: DownloadTask['source']
  sourceId: string
  path: string
  total: number
  durationMs: number
  quality: DownloadTask['quality']
  bitrate: number
  format: string
  finishedAt: number
}

function readEntries(): LibraryEntry[] {
  if (typeof localStorage === 'undefined') return []
  try {
    const raw = localStorage.getItem(STORE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed.filter((item): item is LibraryEntry => {
      const entry = item as Partial<LibraryEntry>
      return typeof entry?.id === 'string' && typeof entry?.path === 'string' && !!entry.path
    })
  } catch {
    return []
  }
}

export const useLibraryStore = defineStore('library', () => {
  const entries = ref<LibraryEntry[]>(readEntries())

  function persist() {
    if (typeof localStorage === 'undefined') return
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(entries.value))
    } catch {
      // 配额写满时退化为本次会话内可用，不影响播放
    }
  }

  const sorted = computed(() =>
    [...entries.value].sort((a, b) => (b.finishedAt || 0) - (a.finishedAt || 0)),
  )

  function has(id: string) {
    return entries.value.some((entry) => entry.id === id)
  }

  /**
   * 按「同一个音源 + 同一个来源 id」找曲库里已有的条目。
   * 只认 id，不拿标题去猜——同名不同版本（伴奏、Live、电台）是不同资源，
   * 用标题匹配会把它们误判成重复。
   */
  function findEntry(source: DownloadTask['source'], sourceId: string) {
    return entries.value.find((entry) => entry.source === source && entry.sourceId === sourceId)
  }

  /** 下载完成后入曲库；同一个任务重复完成（重试成功）时按 id 覆盖。 */
  function add(task: DownloadTask) {
    if (!task.path) return
    const entry: LibraryEntry = {
      id: task.id,
      title: task.title,
      artists: task.artists,
      album: task.album,
      cover: task.cover,
      source: task.source,
      sourceId: task.sourceId,
      path: task.path,
      total: task.total || 0,
      durationMs: task.durationMs || 0,
      quality: task.quality,
      bitrate: task.bitrate || 0,
      format: task.format,
      finishedAt: task.finishedAt ?? Date.now(),
    }
    const index = entries.value.findIndex((item) => item.id === entry.id)
    if (index >= 0) entries.value.splice(index, 1, entry)
    else entries.value.unshift(entry)
    if (entries.value.length > MAX_ENTRIES) entries.value.length = MAX_ENTRIES
    persist()
  }

  function remove(id: string) {
    entries.value = entries.value.filter((entry) => entry.id !== id)
    persist()
  }

  function clear() {
    entries.value = []
    persist()
  }

  return { entries, sorted, has, findEntry, add, remove, clear }
})
