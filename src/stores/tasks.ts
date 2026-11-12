import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { sourceShort, t } from '@/lib/i18n'
import { transport } from '@/lib/transport'
import type { DownloadTask, EngineEvent, EnqueueInput, QualityLevel, Track, Variant } from '@/lib/types'
import { bestVariant, fallbacksOf } from '@/lib/track'
import { useAccountsStore } from '@/stores/accounts'
import { useLibraryStore } from '@/stores/library'

export type TaskFilter = 'all' | 'active' | 'done'

export const useTasksStore = defineStore('tasks', () => {
  const tasks = ref<DownloadTask[]>([])
  const filter = ref<TaskFilter>('all')
  const notices = ref<Array<{ id: number; level: string; message: string }>>([])
  let off: (() => void) | null = null
  let noticeSeq = 0

  const counts = computed(() => {
    const active = tasks.value.filter((task) =>
      ['queued', 'resolving', 'downloading', 'tagging', 'paused'].includes(task.state),
    ).length
    return {
      all: tasks.value.length,
      active,
      done: tasks.value.length - active,
    }
  })

  const visible = computed(() => {
    if (filter.value === 'active') {
      return tasks.value.filter((task) =>
        ['queued', 'resolving', 'downloading', 'tagging', 'paused'].includes(task.state),
      )
    }
    if (filter.value === 'done') {
      return tasks.value.filter((task) =>
        ['completed', 'failed', 'canceled'].includes(task.state),
      )
    }
    return tasks.value
  })

  const totalSpeed = computed(() =>
    tasks.value.reduce((sum, task) => sum + (task.state === 'downloading' ? task.speedBps : 0), 0),
  )

  function pushNotice(level: string, message: string) {
    noticeSeq += 1
    const id = noticeSeq
    notices.value = [...notices.value, { id, level, message }]
    window.setTimeout(() => {
      notices.value = notices.value.filter((item) => item.id !== id)
    }, 4200)
  }

  function handle(event: EngineEvent) {
    if (event.type === 'taskUpsert') {
      const index = tasks.value.findIndex((task) => task.id === event.payload.id)
      if (index >= 0) tasks.value.splice(index, 1, event.payload)
      else tasks.value.unshift(event.payload)
      // 下载完成的文件进曲库；曲库与下载记录分离，清空记录不会清空曲库。
      if (event.payload.state === 'completed' && event.payload.path) {
        useLibraryStore().add(event.payload)
      }
      // 失败原因是「需要登录」而账号库说这个源是登录着的，多半是凭据过期了：
      // 记一笔过期，账号面板会提示重登，下一次取链接也不会再白等一轮。
      if (event.payload.state === 'failed' && event.payload.failReason === 'needLogin') {
        useAccountsStore().markStale(event.payload.resolvedSource ?? event.payload.source)
      }
      return
    }
    if (event.type === 'taskRemoved') {
      tasks.value = tasks.value.filter((task) => task.id !== event.payload)
      return
    }
    if (event.type === 'notice') {
      pushNotice(event.payload.level, event.payload.message)
    }
  }

  async function init() {
    tasks.value = await transport.tasks()
    off?.()
    off = transport.onEvent(handle)
  }

  /**
   * 批量入队。`quality` 用于歌单等批量场景：调用方在界面上选好音质后，
   * 每一首都按这一档提交；不传则沿用设置里的默认档。
   */
  async function enqueue(tracks: Track[], quality?: QualityLevel) {
    if (!tracks.length) return []
    const inputs: EnqueueInput[] = []
    for (const track of tracks) {
      try {
        const variant = bestVariant(track)
        inputs.push({
          source: variant.source,
          sourceId: variant.id,
          title: track.title,
          artists: track.artists,
          album: track.album,
          cover: track.cover,
          durationMs: track.durationMs,
          bitrate: variant.bitrate || undefined,
          format: variant.format,
          quality,
          fallbacks: fallbacksOf(track, variant.source),
        })
      } catch {
        /* 无可用音源的条目直接跳过 */
      }
    }
    const ids = await transport.enqueue(inputs)
    if (inputs.length) {
      pushNotice('info', t('tasks.enqueuedMany', { n: inputs.length }))
    }
    return ids
  }

  /**
   * 用户在下载选择弹窗里明确指定了平台与音质，按该选择入队，
   * 其余平台作为兜底（首选源取不到链接时依次尝试）。
   */
  async function enqueuePicked(
    track: Track,
    variant: Variant,
    quality: QualityLevel,
    label?: string,
  ) {
    const input: EnqueueInput = {
      source: variant.source,
      sourceId: variant.id,
      title: track.title,
      artists: track.artists,
      album: track.album,
      cover: track.cover,
      durationMs: track.durationMs,
      bitrate: variant.bitrate || undefined,
      format: variant.format,
      quality,
      fallbacks: fallbacksOf(track, variant.source),
    }
    const ids = await transport.enqueue([input])
    // 明确回到用户刚才选了什么，否则看起来像「点了没反应、还是默认档」
    const tier = label ? ` · ${label}` : ''
    pushNotice('info', t('tasks.enqueued', { tier, source: sourceShort(variant.source), title: track.title }))
    return ids
  }

  async function pause(id: string) {
    await transport.pause(id)
  }
  async function resume(id: string) {
    await transport.resume(id)
  }
  async function remove(id: string) {
    await transport.remove(id)
  }
  async function clearFinished() {
    const removed = await transport.clearFinished()
    tasks.value = tasks.value.filter(
      (task) => !['completed', 'failed', 'canceled'].includes(task.state),
    )
    if (removed) pushNotice('info', t('tasks.cleared', { n: removed }))
  }
  async function reveal(path: string) {
    try {
      await transport.reveal(path)
    } catch (err) {
      // 移动端没有"定位到文件"的能力，这里降级成提示而不是抛错
      pushNotice('error', err instanceof Error ? err.message : String(err))
    }
  }

  const completed = computed(() => tasks.value.filter((task) => task.state === 'completed'))

  return {
    tasks,
    filter,
    notices,
    counts,
    visible,
    totalSpeed,
    completed,
    init,
    enqueue,
    enqueuePicked,
    pause,
    resume,
    remove,
    clearFinished,
    reveal,
    pushNotice,
  }
})
