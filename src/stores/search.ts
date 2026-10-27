import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { transport } from '@/lib/transport'
import type { AggregatedPage, SourceId, SourceReport, Track } from '@/lib/types'

const PAGE_SIZE = 20

/** 结果排序口径。「热度」用平台人气值（目前网易云给，其它源缺省按 0 排在后面）。 */
export type SortMode = 'default' | 'hot' | 'duration' | 'sources'

export const SORT_MODES: { value: SortMode; label: string; hint: string }[] = [
  { value: 'default', label: '默认', hint: '按平台相关度合并后的原始顺序' },
  { value: 'hot', label: '热度', hint: '按平台人气值从高到低（没有热度数据的排在后面）' },
  { value: 'duration', label: '时长', hint: '短的在前，找试听片段方便' },
  { value: 'sources', label: '音源数', hint: '可下载平台多的在前，成功率更高' },
]

const PREF_KEY = 'melodex.search.prefs'

function isSortMode(value: unknown): value is SortMode {
  return value === 'default' || value === 'hot' || value === 'duration' || value === 'sources'
}

function readPrefs(): SortMode {
  try {
    const raw = localStorage.getItem(PREF_KEY)
    if (raw) {
      // 老版本还存了 pin（置顶原唱），现在这个开关已经删掉，读出来直接忽略
      const parsed = JSON.parse(raw) as { sort?: unknown }
      if (isSortMode(parsed.sort)) return parsed.sort
    }
  } catch {
    // 隐私模式或存储被禁：用默认值，不影响功能
  }
  return 'default'
}

export const useSearchStore = defineStore('search', () => {
  const keyword = ref('')
  const activeKeyword = ref('')
  const sources = ref<SourceId[]>(['netease', 'qq', 'kuwo'])
  const tracks = ref<Track[]>([])
  const reports = ref<SourceReport[]>([])
  const page = ref(1)
  const loading = ref(false)
  const loadingMore = ref(false)
  const error = ref<string | null>(null)
  const elapsed = ref(0)
  const searched = ref(false)

  const sortMode = ref<SortMode>(readPrefs())

  const hasMore = computed(() => tracks.value.length > 0 && tracks.value.length >= page.value * PAGE_SIZE)

  /**
   * 真正用于渲染的顺序。
   *
   * `Array.sort` 是稳定排序，同分保持后端给的原始相对顺序。
   */
  const visible = computed(() => {
    const list = [...tracks.value]
    switch (sortMode.value) {
      case 'hot':
        list.sort((a, b) => (b.popularity ?? 0) - (a.popularity ?? 0))
        break
      case 'duration':
        list.sort((a, b) => a.durationMs - b.durationMs)
        break
      case 'sources':
        list.sort((a, b) => b.variants.length - a.variants.length)
        break
      default:
        break
    }
    return list
  })

  function savePrefs() {
    try {
      localStorage.setItem(PREF_KEY, JSON.stringify({ sort: sortMode.value }))
    } catch {
      // 存不下就只在本次会话生效
    }
  }

  function setSort(next: SortMode) {
    sortMode.value = next
    savePrefs()
  }

  async function run(nextPage = 1, append = false) {
    const query = keyword.value.trim()
    if (!query) {
      tracks.value = []
      reports.value = []
      searched.value = false
      return
    }
    if (append) loadingMore.value = true
    else {
      loading.value = true
      page.value = 1
      error.value = null
    }
    const started = performance.now()
    try {
      const result: AggregatedPage = await transport.search({
        keyword: query,
        page: nextPage,
        limit: PAGE_SIZE,
        sources: sources.value,
      })
      page.value = nextPage
      activeKeyword.value = query
      tracks.value = append ? [...tracks.value, ...result.tracks] : result.tracks
      reports.value = result.reports
      elapsed.value = Math.round(performance.now() - started)
      searched.value = true
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
    } finally {
      loading.value = false
      loadingMore.value = false
    }
  }

  function toggleSource(source: SourceId) {
    const next = sources.value.includes(source)
      ? sources.value.filter((item) => item !== source)
      : [...sources.value, source]
    if (!next.length) return
    sources.value = next
    if (searched.value) void run(1, false)
  }

  function reset() {
    keyword.value = ''
    activeKeyword.value = ''
    tracks.value = []
    reports.value = []
    searched.value = false
  }

  return {
    keyword,
    activeKeyword,
    sources,
    tracks,
    reports,
    page,
    loading,
    loadingMore,
    error,
    elapsed,
    searched,
    hasMore,
    sortMode,
    visible,
    setSort,
    run,
    toggleSource,
    reset,
  }
})
