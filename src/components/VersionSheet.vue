<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { comparisonTrack as track } from '@/lib/version-sheet'
import { formatDuration, formatBitrate } from '@/lib/format'
import { sourceShort, useI18n } from '@/lib/i18n'
import { usePlayerStore } from '@/stores/player'
import type { Variant } from '@/lib/types'
import Icon from './Icon.vue'
const { t } = useI18n(), player = usePlayerStore()
const dialog = ref<HTMLDialogElement>()
watch(track, async value => { if(value){await nextTick();if(!dialog.value?.open)dialog.value?.showModal()} })
function listen(v:Variant){if(track.value)void player.playTrack(track.value,v);dialog.value?.close()}
</script>
<template><dialog ref="dialog" class="versions-sheet" aria-labelledby="version-heading" @close="track=null" @click.self="dialog?.close()">
<section v-if="track"><header><div><small>{{t('versions.compare')}}</small><h2 id="version-heading">{{track.title}}</h2></div><button class="icon-btn" :aria-label="t('common.close')" @click="dialog?.close()"><Icon name="close" :size="18"/></button></header>
<button class="version-choice" v-for="v in track.variants" :key="v.source+v.id" @click="listen(v)"><span><b>{{sourceShort(v.source)}} · {{v.title}}</b><small>{{[v.album,formatDuration(v.durationMs),v.format,v.bitrate?formatBitrate(v.bitrate):''].filter(Boolean).join(' · ')}}</small></span><Icon name="play" :size="18"/></button>
</section></dialog></template>
<style scoped>.versions-sheet{position:fixed;inset:auto 0 0;margin:0 auto;width:min(100%,520px);max-height:76dvh;overflow:auto;padding:18px 18px max(18px,env(safe-area-inset-bottom));background:var(--bg);color:var(--text);border:1px solid var(--line);border-radius:24px 24px 0 0}.versions-sheet::backdrop{background:#17203455;backdrop-filter:blur(4px)}header,.version-choice{display:flex;align-items:center;justify-content:space-between;gap:12px}h2{margin:3px 0 12px}small{display:block;color:var(--text-2)}.version-choice{width:100%;text-align:left;padding:12px;border:0;border-bottom:1px solid var(--line);background:transparent;color:inherit}.version-choice svg{flex:none;color:var(--accent)}.version-choice span{min-width:0;overflow-wrap:anywhere}</style>