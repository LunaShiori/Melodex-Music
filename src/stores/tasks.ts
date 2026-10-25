import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { transport } from '@/lib/transport'
import type { DownloadTask, EngineEvent, EnqueueInput, Track } from '@/lib/types'
import { bestVariant } from '@/lib/track'

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

  async function enqueue(tracks: Track[]) {
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
        })
      } catch {
        /* 无可用音源的条目直接跳过 */
      }
    }
    const ids = await transport.enqueue(inputs)
    if (inputs.length) {
      pushNotice('info', `已加入 ${inputs.length} 个下载任务`)
    }
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
    if (removed) pushNotice('info', `已清理 ${removed} 条记录`)
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
    pause,
    resume,
    remove,
    clearFinished,
    reveal,
    pushNotice,
  }
})
