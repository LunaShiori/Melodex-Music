import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { transport } from '@/lib/transport'
import type { AggregatedPage, SourceId, SourceReport, Track } from '@/lib/types'

const PAGE_SIZE = 20

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

  const hasMore = computed(() => tracks.value.length > 0 && tracks.value.length >= page.value * PAGE_SIZE)

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
    run,
    toggleSource,
    reset,
  }
})
