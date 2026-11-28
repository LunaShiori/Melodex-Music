<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { useRoute } from 'vue-router'
import { transport } from '@/lib/transport'
import { useI18n } from '@/lib/i18n'
import { reviewTracks } from '@/lib/download-review'
import { QUALITY_LEVELS } from '@/lib/quality'
import type { Track, Variant, QualityLevel } from '@/lib/types'
import TrackRow from '@/components/TrackRow.vue'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'
import { useSettingsStore } from '@/stores/settings'
interface Catalog { id: string; name: string; description: string; cover: string; tracks: Variant[]; complete: boolean }
interface Album { id: string; name: string; cover: string }
const route = useRoute(), { t } = useI18n(), player = usePlayerStore(), tasks = useTasksStore(), settings = useSettingsStore()
const catalogRoute = { params: route.params, query: route.query }
const data = ref<Catalog | null>(null)
const error = ref(''), loading = ref(false), busy = ref(false), queued = ref(false)
const albumError = ref(''), albums = ref<Album[]>([]), albumsBusy = ref(false), more = ref(false)
const quality = ref<QualityLevel>(settings.settings?.qualityOverrides.netease ?? settings.settings?.quality ?? 'high')
const selected = ref(new Set<string>())
let generation = 0, offset = 0
const tracks = computed<Track[]>(() => (data.value?.tracks ?? []).map(v => ({ ...v, fingerprint: v.source + ':' + v.id, variants: [v] })))
const review = computed(() => reviewTracks(tracks.value, tasks.tasks))
async function retryFailed() {
  if (busy.value) return
  const attempt = generation
  busy.value = true
  const results = await Promise.allSettled(review.value.failed.map(r => tasks.resume(r.task!.id)))
  const failed = results.find(r => r.status === 'rejected')
  if (attempt === generation && failed?.status === 'rejected') error.value = String(failed.reason)
  busy.value = false
}
const chosen = computed(() => tracks.value.filter(v => selected.value.has(v.fingerprint)))
async function loadAlbums(current = generation) {
  if (!data.value || albumsBusy.value) return
  const id = data.value.id
  albumsBusy.value = true; albumError.value = ''
  try {
    const result = await transport.catalogAlbums(id, offset)
    if (current !== generation) return
    const existing = new Set(albums.value.map(a => a.id))
    albums.value.push(...result.albums.filter(a => !existing.has(a.id)))
    more.value = result.more && result.offset > offset
    offset = result.offset
  } catch (e) { if (current === generation) albumError.value = String(e) }
  finally { if (current === generation) albumsBusy.value = false }
}
async function load() {
  const current = ++generation
  data.value = null; error.value = ''; loading.value = true; selected.value = new Set(); queued.value = false
  albums.value = []; albumError.value = ''; albumsBusy.value = false; more.value = false; offset = 0
  try {
    const result = await transport.catalogDetail({ songId: String(catalogRoute.query.songId ?? ''), kind: String(catalogRoute.params.kind), catalogId: typeof catalogRoute.query.id === 'string' ? catalogRoute.query.id : null, artist: typeof catalogRoute.query.artist === 'string' ? catalogRoute.query.artist : null })
    if (current !== generation) return
    data.value = result
    if (catalogRoute.params.kind === 'artist') void loadAlbums(current)
  } catch (e) { if (current === generation) error.value = String(e) }
  finally { if (current === generation) loading.value = false }
}
onMounted(load)
onBeforeUnmount(() => { generation++ })
function toggle(track: Track) {
  const next = new Set(selected.value)
  if (next.has(track.fingerprint)) next.delete(track.fingerprint); else next.add(track.fingerprint)
  selected.value = next
}
function selectAll() { selected.value = chosen.value.length === tracks.value.length ? new Set() : new Set(tracks.value.map(v => v.fingerprint)) }
async function play(track: Track) { try { await player.playList(tracks.value, tracks.value.findIndex(v => v.fingerprint === track.fingerprint)) } catch (e) { error.value = String(e) } }
async function download(items: Track[], whole = false) {
  if (busy.value || !items.length) return
  const current = generation
  busy.value = true; error.value = ''
  try { const ids = await tasks.enqueue(items, quality.value); if (whole && current === generation && ids.length) queued.value = true }
  catch (e) { if (current === generation) error.value = String(e) }
  finally { busy.value = false }
}
</script>
<template>
  <div class="catalog">
    <p v-if="transport.mode === 'demo'" class="muted">{{ t('catalog.demo') }}</p>
    <button class="btn" type="button" @click="$router.options.history.state.back ? $router.back() : $router.replace('/search')">← {{ t('nav.search') === 'Search' ? 'Back' : '返回' }}</button>
    <p v-if="loading" role="status">{{ t('common.loading') }}</p>
    <div v-if="error" class="error" role="alert"><p>{{ error }}</p><button v-if="!data" class="btn" @click="load">{{ t('catalog.retry') }}</button> <RouterLink to="/settings">{{ t('catalog.login') }}</RouterLink></div>
    <template v-if="data">
      <header><img v-if="data.cover" :src="data.cover" alt="" /><div><p class="muted">{{ t(catalogRoute.params.kind === 'album' ? 'catalog.album' : 'catalog.artist') }} · {{ t('catalog.source') }}</p><h1>{{ data.name }}</h1><p>{{ tracks.length }} {{ t('catalog.songs') }}</p></div></header>
      <details v-if="data.description"><summary>{{ t('catalog.about') }}</summary><p>{{ data.description }}</p></details>
      <p v-if="!data.complete" class="muted">{{ t('catalog.partial') }}</p>
      <div class="actions">
        <label>{{ t('playlists.batchQuality') }} <select v-model="quality"><option v-for="level in QUALITY_LEVELS" :key="level.id" :value="level.id">{{ t(level.label) }}</option></select></label>
        <button v-if="data.complete" class="btn btn-primary" :disabled="busy || !tracks.length" @click="download(tracks, true)">{{ t('catalog.downloadAll') }}</button>
        <button class="btn" :disabled="!tracks.length" @click="selectAll">{{ t('catalog.selectAll') }}</button>
        <button v-if="chosen.length" class="btn" :disabled="busy" @click="download(chosen)">{{ t('catalog.downloadSelected', { n: chosen.length }) }}</button>
        <RouterLink v-if="queued" to="/downloads">{{ t('nav.downloads') }}</RouterLink>
      </div>
      <section v-if="tracks.length" class="download-review" aria-live="polite">
        <strong>{{ t('downloadReview.summary') }}</strong>
        <p>{{ t('downloadReview.counts', { done: review.completed, active: review.active, failed: review.failed.length, missing: review.missing.length }) }}</p>
        <div class="actions"><button v-if="review.failed.length" class="btn" :disabled="busy" @click="retryFailed">{{ t('downloadReview.retry') }}</button><button v-if="review.missing.length" class="btn" :disabled="busy" @click="download(review.missing.map(r=>r.track))">{{ t('downloadReview.missing') }}</button></div>
      </section>
      <p class="muted">{{ t('catalog.rights') }}</p>
      <p v-if="!tracks.length">{{ t('catalog.empty') }}</p>
      <div v-for="(track, index) in tracks" :key="track.fingerprint" class="track-item">
        <input type="checkbox" :aria-label="t('catalog.selectTrack', { title: track.title })" :checked="selected.has(track.fingerprint)" @change="toggle(track)" />
        <TrackRow :track="track" :index="index" :active="player.isCurrent(track)" :playing="player.playing" @play="play" @download="download([$event])" />
      </div>
      <section v-if="catalogRoute.params.kind === 'artist'">
        <h2>{{ t('catalog.albums') }}</h2>
        <div class="albums"><RouterLink v-for="album in albums" :key="album.id" class="album-card" :to="{ name: 'catalog', params: { kind: 'album' }, query: { id: album.id } }"><img v-if="album.cover" :src="album.cover" alt="" loading="lazy" /><span>{{ album.name }}</span></RouterLink></div>
        <p v-if="albumError" role="alert">{{ albumError }}</p>
        <button v-if="more || albumError" class="btn" :disabled="albumsBusy" @click="loadAlbums()">{{ t(albumError ? 'catalog.retry' : 'catalog.more') }}</button>
        <p v-if="albumsBusy" role="status">{{ t('common.loading') }}</p>
      </section>
    </template>
  </div>
</template>
<style scoped>
.download-review { padding: 14px; margin: 12px 0; border: 1px solid var(--line); border-radius: 14px; background: var(--surface); } .download-review p { margin: 8px 0; } .download-review .actions { margin: 8px 0 0; }
.catalog { padding: var(--view-pad); min-width: 0; overflow-wrap: anywhere; padding-bottom: calc(24px + env(safe-area-inset-bottom, 0px)); }
header { display: flex; gap: 20px; align-items: center; margin: 12px 0; } header img { width: 96px; height: 96px; border-radius: 14px; object-fit: cover; flex-shrink: 0; } header div { min-width: 0; } h1 { font-size: 24px; } p { line-height: 1.6; } .actions { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin: 16px 0; } select { max-width: 100%; padding: 8px; color: inherit; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; } .track-item { display: grid; grid-template-columns: 20px minmax(0, 1fr); align-items: center; } input[type=checkbox] { accent-color: var(--accent); } section { margin-top: 28px; } .albums { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 16px; margin: 16px 0; } .album-card { min-width: 0; display: flex; flex-direction: column; gap: 8px; color: inherit; text-decoration: none; } .album-card img { width: 100%; aspect-ratio: 1; border-radius: 12px; object-fit: cover; } .error { color: var(--danger, #c54c4c); } details { margin: 12px 0; }
@media (max-width: 480px) { header { gap: 12px; } header img { width: 72px; height: 72px; } h1 { font-size: 21px; } }
</style>
