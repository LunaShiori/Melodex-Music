<script setup lang="ts">
import { useI18n } from '@/lib/i18n'
const { t } = useI18n()
</script>
<template><nav class="hub-tabs" :aria-label="t('nav.musicHub')"><RouterLink to="/library">{{ t('nav.library') }}</RouterLink><RouterLink to="/recent">{{ t('nav.recent') }}</RouterLink><RouterLink to="/playlists">{{ t('nav.playlists') }}</RouterLink></nav></template>
<style scoped>.hub-tabs{display:flex;gap:8px;margin-bottom:18px;flex-wrap:wrap}.hub-tabs a{padding:9px 16px;border-radius:18px;color:var(--text-2);text-decoration:none;background:var(--surface);border:1px solid var(--line);transition:background .2s,transform .2s}.hub-tabs a.router-link-active{background:var(--accent-soft);color:var(--accent);border-color:var(--accent)}.hub-tabs a:active{transform:scale(.96)}</style>
