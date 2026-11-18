<script setup lang="ts">
import { nextTick, ref, watch, onBeforeUnmount } from 'vue'
import { useTasksStore } from '@/stores/tasks'
import { useI18n } from '@/lib/i18n'
const tasks = useTasksStore(), { t } = useI18n()
const dialog = ref<HTMLElement | null>(null)
let previous: HTMLElement | null = null
watch(() => tasks.duplicateReview.length > 0, async visible => {
  if (visible) { previous = document.activeElement as HTMLElement; await nextTick(); dialog.value?.querySelector('button')?.focus() }
  else if (previous?.isConnected) previous.focus()
})
function trap(event: KeyboardEvent) {
  if (event.key !== 'Tab') return
  const buttons = [...(dialog.value?.querySelectorAll('button') ?? [])]
  const first = buttons[0], last = buttons[buttons.length - 1]
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus() }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus() }
}
onBeforeUnmount(() => tasks.decideDuplicates('cancel'))
</script>
<template><Teleport to="body"><div v-if="tasks.duplicateReview.length" class="duplicate-mask" @keydown.esc="tasks.decideDuplicates('cancel')"><section ref="dialog" class="duplicate-dialog" @keydown="trap" role="dialog" aria-modal="true" aria-labelledby="duplicate-title"><h2 id="duplicate-title">{{ t('downloadReview.title') }}</h2><p>{{ t('downloadReview.hint') }}</p><ul><li v-for="(item,index) in tasks.duplicateReview" :key="index">{{ item.title }} <small>{{ item.format.toUpperCase() }} · {{ item.bitrate }} kbps</small></li></ul><footer><button class="btn" autofocus @click="tasks.decideDuplicates('cancel')">{{ t('downloadReview.cancel') }}</button><button class="btn" @click="tasks.decideDuplicates('all')">{{ t('downloadReview.again') }}</button><button class="btn btn-primary" @click="tasks.decideDuplicates('skip')">{{ t('downloadReview.skip') }}</button></footer></section></div></Teleport></template>
<style scoped>.duplicate-mask{position:fixed;inset:0;z-index:220;background:#0007;display:grid;place-items:center;padding:24px}.duplicate-dialog{background:var(--surface);color:var(--text);padding:24px;border:1px solid var(--line);border-radius:20px;width:min(480px,100%);max-height:80dvh;overflow:auto;overflow-wrap:anywhere}.duplicate-dialog p{line-height:1.6;color:var(--text-2)}ul{padding-left:20px;max-height:35dvh;overflow:auto}li{padding:8px 0}small{display:block;color:var(--text-2)}footer{display:flex;flex-wrap:wrap;gap:8px;justify-content:flex-end}</style>
