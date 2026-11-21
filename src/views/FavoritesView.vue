<script setup lang="ts">
import { useRoute } from 'vue-router'
import { computed, ref, onMounted } from 'vue'
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
const collection = computed(() => selected.value ? favorites.lists.find(list => list.name === selected.value)?.tracks ?? [] : favorites.tracks)
const query = ref('')
const artist = ref('')
const artists = computed(() => [...new Set(collection.value.flatMap(track => track.artists))].sort((a,b) => a.localeCompare(b)))
const visible = computed(() => {
  const term = query.value.trim().toLocaleLowerCase()
  return collection.value.filter(track => (!artist.value || track.artists.includes(artist.value)) &&
    [track.title, track.album, ...track.artists].join(' ').toLocaleLowerCase().includes(term))
})
function shufflePlay() {
  const queue = [...visible.value]
  for (let i = queue.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    const item = queue[i]!
    queue[i] = queue[j]!
    queue[j] = item
  }
  if (queue.length) player.playList(queue, 0)
}
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
const backupText = ref('')
const backupFile = ref<HTMLInputElement|null>(null)
type BackupList = {name:string; tracks:Track[]}
const pendingBackup = ref<{tracks:Track[]; lists:BackupList[]}|null>(null)
function exportBackup(){
  const blob=new Blob([JSON.stringify({format:'melodex-library-1',tracks:favorites.tracks,lists:favorites.lists})],{type:'application/json'})
  const url=URL.createObjectURL(blob), link=document.createElement('a')
  link.href=url;link.download='melodex-library.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),30000)
}
function validTrack(t:any): t is Track {
  return typeof t?.fingerprint==='string' && typeof t.title==='string' && Array.isArray(t.artists) && t.artists.every((a:unknown)=>typeof a==='string') && Array.isArray(t.variants) && t.variants.length>0 && t.variants.every((v:any)=>v && ALL_SOURCES.includes(v.source) && typeof v.id==='string' && Array.isArray(v.artists) && typeof v.title==='string')
}
async function previewBackup(file?:File){
  pendingBackup.value=null
  if(!file)return
  try{
    if(file.size>20*1024*1024)throw Error()
    const data=JSON.parse(await file.text())
    if(data.format!=='melodex-library-1'||!Array.isArray(data.tracks)||!Array.isArray(data.lists))throw Error()
    let skipped=0
    const clean=(items:unknown[])=>items.filter((item):item is Track=>{if(validTrack(item))return true;skipped++;return false})
    const tracks=clean(data.tracks), lists:BackupList[]=[]
    for(const list of data.lists){if(typeof list?.name==='string' && list.name.trim() && Array.isArray(list.tracks))lists.push({name:list.name,tracks:clean(list.tracks)});else skipped++}
    const fresh=new Set(tracks.filter(t=>!favorites.ids.has(t.fingerprint)).map(t=>t.fingerprint)).size
    backupText.value='新增收藏 '+fresh+' · 重复 '+(tracks.length-fresh)+' · 歌单 '+lists.length+' · 无法识别 '+skipped
    pendingBackup.value={tracks,lists}
  }catch{backupText.value='无法读取这个备份文件'}
  if(backupFile.value)backupFile.value.value=''
}
function mergeBackup(){
  const data=pendingBackup.value;if(!data)return
  try{for(const track of [...data.tracks].reverse())if(!favorites.ids.has(track.fingerprint))favorites.toggle(track)
    for(const list of data.lists)favorites.saveList(list.name,list.tracks)
    pendingBackup.value=null;backupText.value='已合并到音乐库'
  }catch{backupText.value='存储空间不足，可重试合并'}
}
const editor = ref<HTMLDetailsElement | null>(null)
const route=useRoute()
onMounted(()=>{if(route.query.match==='1'){input.value=sessionStorage.getItem('melodex.matchDraft')||'';if(editor.value)editor.value.open=true;if(input.value)void match()}})
const confirmed = computed(() => matches.value.flatMap(row => row.choices[row.pick] ? [row.choices[row.pick]!] : []))
</script>
<template><main class="favorites"><MusicHubTabs/><h1>{{ t('fav.title') }}</h1>
<details class="list-tools"><summary>歌单搬家与备份</summary><button class="btn" @click="exportBackup">导出收藏和全部歌单</button><label>选择备份预览<input ref="backupFile" type="file" accept=".json" @change="previewBackup(($event.target as HTMLInputElement).files?.[0])"/></label><p role="status">{{backupText}}</p><button v-if="pendingBackup" class="btn" @click="mergeBackup">确认合并</button></details><div class="list-tools"><select v-model="selected" @change="artist='';query=''" :aria-label="t('nav.playlists')"><option value="">{{ t('fav.title') }}</option><option v-for="list in favorites.lists" :key="list.name">{{ list.name }}</option></select>
<button class="btn" v-if="visible.length" @click="copyTrackList">{{ t('lists.copy') }}</button><details ref="editor"><summary>{{ t('lists.manage') }}</summary><label>{{ t('lists.name') }} <input v-model="name" maxlength="80" /></label>
<button class="btn" :disabled="!name.trim() || !favorites.tracks.length" @click="save(favorites.tracks)">{{ t('lists.fromFavorites') }}</button>
<label>{{ t('lists.paste') }}<textarea v-model="input" :disabled="busy" @input="matches=[]" rows="4" /></label>
<button class="btn" :disabled="busy || !input.trim()" @click="match">{{ t(busy ? 'common.loading' : 'lists.match') }}</button>
<label v-for="row in matches" :key="row.query">{{ row.query }}<select v-model.number="row.pick"><option :value="-1">{{ t(row.choices.length ? 'lists.skip' : 'lists.noMatch') }}</option><option v-for="(item,i) in row.choices" :value="i" :key="item.fingerprint">{{ item.title }} · {{ item.artists.join(' / ') }} · {{ item.album }}</option></select><button v-if="row.choices[row.pick]" class="btn" @click="player.playTrack(row.choices[row.pick]!)">试听所选版本</button></label>
<button v-if="matches.length" class="btn" :disabled="busy || !confirmed.length || !name.trim()" @click="save(confirmed)">{{ t('lists.save') }} ({{ confirmed.length }})</button><p role="status">{{ message }}</p></details></div>
<section v-if="collection.length" class="mix"><h2>{{ t('mix.title') }}</h2>
<div><input type="search" v-model="query" :placeholder="t('mix.find')" :aria-label="t('mix.find')"/>
<select v-model="artist" :aria-label="t('mix.artists')"><option value="">{{ t('mix.artists') }}</option><option v-for="a in artists" :key="a">{{ a }}</option></select></div>
<div><button class="btn" :disabled="!visible.length" @click="shufflePlay">{{ t('mix.shuffle') }} · {{ visible.length }}</button>
<button class="btn" :disabled="!visible.length" @click="tasks.enqueue([...visible])">{{ t('mix.download') }}</button></div>
<form @submit.prevent="save([...visible]);artist='';query=''"><input v-model="name" maxlength="80" :placeholder="t('lists.name')" :aria-label="t('lists.name')"/><button class="btn" :disabled="!name.trim() || !visible.length">{{ t('lists.save') }}</button></form></section>
<p v-if="!visible.length">{{ t('fav.empty') }}</p>
<TrackRow v-for="(track,index) in visible" :key="track.fingerprint" :track="track" :index="index" :active="player.isCurrent(track)" :playing="player.playing" @play="player.playList([...visible],index)" @download="tasks.enqueue([track])"/>
</main></template>
<style scoped>.mix{display:grid;gap:14px;padding:20px;border:1px solid var(--line);border-radius:18px;background:var(--surface);margin-bottom:16px}.mix h2{margin:0}.mix div,.mix form{display:flex;flex-wrap:wrap;gap:8px}.mix input,.mix select{max-width:100%;min-width:0;padding:8px;border-radius:8px;border:1px solid var(--line);background:var(--surface);color:var(--text)}.list-tools{display:grid;gap:12px;margin:16px 0}.list-tools label{display:grid;gap:6px;margin:12px 0}.list-tools select,.list-tools input,.list-tools textarea{max-width:100%;color:var(--text);background:var(--surface);padding:9px;border:1px solid var(--line);border-radius:8px}.list-tools details{padding:14px;background:var(--surface);border-radius:12px}.favorites{padding:var(--view-pad);max-width:1280px;margin:auto}h1{margin:18px 0}</style>
