<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Track } from '@/lib/types'
import { transport } from '@/lib/transport'
import { useSettingsStore } from '@/stores/settings'
import { ALL_SOURCES } from '@/lib/types'
import MusicHubTabs from '@/components/MusicHubTabs.vue'
import TrackRow from '@/components/TrackRow.vue'
import { useFavorites } from '@/stores/favorites'
import { usePlayerStore } from '@/stores/player'
import { useTasksStore } from '@/stores/tasks'
import { useI18n } from '@/lib/i18n'
const favorites = useFavorites(), player = usePlayerStore(), tasks = useTasksStore()
const { t } = useI18n()
const settings = useSettingsStore()
const selected = ref(''), name = ref(''), input = ref(''), busy = ref(false)
const message = ref('')
const matches = ref<{ query: string; choices: Track[]; pick: number }[]>([])
const visible = computed(() => selected.value ? favorites.lists.find(list => list.name === selected.value)?.tracks ?? [] : favorites.tracks)
async function match() {
  const lines = [...new Set(input.value.split(/\r?\n/).map(line => line.trim()).filter(Boolean))]
  if (!lines.length || busy.value) return
  if (lines.length > 100) { message.value = t('lists.limit'); return }
  busy.value = true
  matches.value = []
  message.value = ''
  try {
    for (const query of lines) {
      try {
        const result = await transport.search({ keyword: query, sources: settings.settings?.sources ?? ALL_SOURCES, page: 1, limit: 5 })
        matches.value.push({ query, choices: result.tracks.slice(0,5), pick: -1 })
      } catch { matches.value.push({ query, choices: [], pick: -1 }) }
    }
  } finally { busy.value = false }
}
function save(items: Track[]) {
  if (!name.value.trim() || !items.length) return
  try {
    favorites.saveList(name.value.trim(), items)
    selected.value = name.value.trim()
    message.value = t('lists.saved')
  } catch { message.value = t('fav.error') }
}
async function copyTrackList() {
  const lines = visible.value.map(track => [track.title, ...track.artists].join(' '))
  try {
    await navigator.clipboard.writeText(lines.join('\n'))
    tasks.pushNotice('info', t('lists.copied'))
  } catch {
    input.value = lines.join('\n')
    matches.value = []
    editor.value!.open = true
    tasks.pushNotice('info', t('lists.copyManual'))
  }
}
const editor = ref<HTMLDetailsElement | null>(null)
const confirmed = computed(() => matches.value.flatMap(row => row.choices[row.pick] ? [row.choices[row.pick]!] : []))
</script>
<template><main class="favorites"><MusicHubTabs/><h1>{{ t('fav.title') }}</h1>
<div class="list-tools"><select v-model="selected" :aria-label="t('nav.playlists')"><option value="">{{ t('fav.title') }}</option><option v-for="list in favorites.lists" :key="list.name">{{ list.name }}</option></select>
<button class="btn" v-if="visible.length" @click="copyTrackList">{{ t('lists.copy') }}</button><details ref="editor"><summary>{{ t('lists.manage') }}</summary><label>{{ t('lists.name') }} <input v-model="name" maxlength="80" /></label>
<button class="btn" :disabled="!name.trim() || !favorites.tracks.length" @click="save(favorites.tracks)">{{ t('lists.fromFavorites') }}</button>
<label>{{ t('lists.paste') }}<textarea v-model="input" :disabled="busy" @input="matches=[]" rows="4" /></label>
<button class="btn" :disabled="busy || !input.trim()" @click="match">{{ t(busy ? 'common.loading' : 'lists.match') }}</button>
<label v-for="row in matches" :key="row.query">{{ row.query }}<select v-model.number="row.pick"><option :value="-1">{{ t(row.choices.length ? 'lists.skip' : 'lists.noMatch') }}</option><option v-for="(item,i) in row.choices" :value="i" :key="item.fingerprint">{{ item.title }} · {{ item.artists.join(' / ') }} · {{ item.album }}</option></select></label>
<button v-if="matches.length" class="btn" :disabled="busy || !confirmed.length || !name.trim()" @click="save(confirmed)">{{ t('lists.save') }} ({{ confirmed.length }})</button><p role="status">{{ message }}</p></details></div>
<p v-if="!visible.length">{{ t('fav.empty') }}</p>
<TrackRow v-for="(track,index) in visible" :key="track.fingerprint" :track="track" :index="index" :active="player.isCurrent(track)" :playing="player.playing" @play="player.playList([...visible],index)" @download="tasks.enqueue([track])"/>
</main></template>
<style scoped>.list-tools{display:grid;gap:12px;margin:16px 0}.list-tools label{display:grid;gap:6px;margin:12px 0}.list-tools select,.list-tools input,.list-tools textarea{max-width:100%;color:var(--text);background:var(--surface);padding:9px;border:1px solid var(--line);border-radius:8px}.list-tools details{padding:14px;background:var(--surface);border-radius:12px}.favorites{padding:var(--view-pad);max-width:1280px;margin:auto}h1{margin:18px 0}</style>
