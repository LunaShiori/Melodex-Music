import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import type { Track } from '@/lib/types'

const key = 'melodex.favorites.v1'
function load(): Track[] {
  try {
    const data = JSON.parse(localStorage.getItem(key) ?? '[]')
    return Array.isArray(data) ? data.filter(t => typeof t?.fingerprint === 'string' && Array.isArray(t.artists) && Array.isArray(t.variants) && t.variants.length) : []
  } catch { return [] }
}

export const useFavorites = defineStore('favorites', () => {
  const tracks = ref<Track[]>(load())
  const ids = computed(() => new Set(tracks.value.map(track => track.fingerprint)))
  function toggle(track: Track) {
    const next = ids.value.has(track.fingerprint)
      ? tracks.value.filter(item => item.fingerprint !== track.fingerprint)
      : [track, ...tracks.value]
    localStorage.setItem(key, JSON.stringify(next))
    tracks.value = next
  }
  const listsKey = 'melodex.localLists.v1'
  type LocalList = { name: string; tracks: Track[] }
  function readLists(): LocalList[] {
    try {
      const data = JSON.parse(localStorage.getItem(listsKey) ?? '[]')
      return Array.isArray(data) ? data.filter(list => typeof list?.name === 'string' && Array.isArray(list.tracks)) : []
    } catch { return [] }
  }
  const lists = ref<LocalList[]>(readLists())
  function saveList(name: string, items: Track[]) {
    const existing = lists.value.find(list => list.name === name)
    const merged = new Map((existing?.tracks ?? []).map(track => [track.fingerprint, track]))
    for (const item of items) merged.set(item.fingerprint, item)
    const next = [...lists.value.filter(list => list.name !== name), { name, tracks: [...merged.values()] }]
    localStorage.setItem(listsKey, JSON.stringify(next))
    lists.value = next
  }
  return { tracks, ids, toggle, lists, saveList }
})
