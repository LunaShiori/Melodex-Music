<script setup lang="ts">
import { computed } from 'vue'

import Icon from '@/components/Icon.vue'
import { coverOf } from '@/lib/artwork'
import { formatBytes, formatRelative, formatSpeed } from '@/lib/format'
import { SOURCE_META } from '@/lib/types'
import type { DownloadTask, TaskState } from '@/lib/types'
import { useTasksStore } from '@/stores/tasks'

const tasks = useTasksStore()

const STATE_TEXT: Record<TaskState, string> = {
  queued: '排队中',
  resolving: '解析链接',
  downloading: '下载中',
  tagging: '写入标签',
  completed: '已完成',
  failed: '失败',
  paused: '已暂停',
  canceled: '已取消',
}

const tabs = computed(() => [
  { key: 'all' as const, label: '全部', count: tasks.counts.all },
  { key: 'active' as const, label: '进行中', count: tasks.counts.active },
  { key: 'done' as const, label: '已结束', count: tasks.counts.done },
])

function ratio(task: DownloadTask) {
  if (!task.total) return task.state === 'tagging' ? 0.99 : 0.08
  return Math.min(1, task.downloaded / task.total)
}

function isActive(task: DownloadTask) {
  return ['queued', 'resolving', 'downloading', 'tagging', 'paused'].includes(task.state)
}
</script>

<template>
  <div class="view">
    <header class="head">
      <div>
        <h1>下载中心</h1>
        <p class="muted">多任务并发、断点续传、音质自动降级，失败可单条重试。</p>
      </div>
      <div class="stats">
        <div class="stat">
          <span class="num mono">{{ tasks.counts.active }}</span>
          <span class="lab">进行中</span>
        </div>
        <div class="stat">
          <span class="num mono">{{ tasks.counts.done }}</span>
          <span class="lab">已结束</span>
        </div>
        <div class="stat">
          <span class="num mono">{{ formatSpeed(tasks.totalSpeed) }}</span>
          <span class="lab">总速度</span>
        </div>
      </div>
    </header>

    <div class="toolbar">
      <div class="tabs">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          class="tab"
          :class="{ on: tasks.filter === tab.key }"
          type="button"
          @click="tasks.filter = tab.key"
        >
          {{ tab.label }}
          <span class="count mono">{{ tab.count }}</span>
        </button>
      </div>
      <button class="btn btn-sm" type="button" :disabled="!tasks.counts.done" @click="tasks.clearFinished()">
        <Icon name="trash" :size="14" />
        清理已结束
      </button>
    </div>

    <section v-if="tasks.visible.length" class="list">
      <article v-for="task in tasks.visible" :key="task.id" class="task" :class="task.state">
        <div class="cover">
          <img :src="coverOf(task.cover, task.id)" alt="" loading="lazy" />
        </div>

        <div class="body">
          <div class="line">
            <p class="title">{{ task.title }}</p>
            <span class="badge" :class="task.state">{{ STATE_TEXT[task.state] }}</span>
            <span class="src" :style="{ color: SOURCE_META[task.source].color }">
              {{ SOURCE_META[task.source].short }}
            </span>
          </div>
          <p class="sub">
            {{ task.artists.join(' / ') || '未知歌手' }}
            <span class="dot">·</span>
            {{ task.bitrate }}k {{ task.format.toUpperCase() }}
            <span class="dot">·</span>
            {{ formatRelative(task.createdAt) }}
          </p>

          <div class="progress" :class="{ done: task.state === 'completed' }">
            <div class="track">
              <div class="fill" :style="{ width: `${ratio(task) * 100}%` }" />
            </div>
            <div class="progress-meta mono">
              <span v-if="task.state === 'downloading'">
                {{ formatSpeed(task.speedBps) }} · {{ formatBytes(task.downloaded) }} /
                {{ formatBytes(task.total) }}
              </span>
              <span v-else-if="task.state === 'completed'">
                {{ formatBytes(task.total || task.downloaded) }} · 已保存
              </span>
              <span v-else-if="task.state === 'failed'" class="err">{{ task.error }}</span>
              <span v-else>{{ STATE_TEXT[task.state] }}</span>
            </div>
          </div>
        </div>

        <div class="actions">
          <button
            v-if="!isActive(task) && task.state !== 'completed'"
            class="icon-btn"
            title="重试"
            type="button"
            @click="tasks.resume(task.id)"
          >
            <Icon name="refresh" :size="16" />
          </button>
          <button
            v-if="task.state === 'downloading' || task.state === 'queued'"
            class="icon-btn"
            title="暂停"
            type="button"
            @click="tasks.pause(task.id)"
          >
            <Icon name="pause" :size="16" />
          </button>
          <button
            v-if="task.state === 'paused'"
            class="icon-btn"
            title="继续"
            type="button"
            @click="tasks.resume(task.id)"
          >
            <Icon name="play" :size="15" />
          </button>
          <button
            v-if="task.state === 'completed' && task.path"
            class="icon-btn"
            title="打开文件位置"
            type="button"
            @click="tasks.reveal(task.path!)"
          >
            <Icon name="folder" :size="16" />
          </button>
          <button class="icon-btn" title="删除任务" type="button" @click="tasks.remove(task.id)">
            <Icon name="close" :size="16" />
          </button>
        </div>
      </article>
    </section>

    <section v-else class="empty">
      <div class="empty-art"><Icon name="download" :size="26" /></div>
      <h3>还没有下载任务</h3>
      <p class="muted">在搜索结果里点右侧的下载按钮，或直接点音源标签从指定平台取文件。</p>
      <RouterLink to="/search" class="btn btn-primary" style="margin-top: 16px">去搜索</RouterLink>
    </section>
  </div>
</template>

<style scoped>
.view {
  padding: 26px 26px 40px;
  max-width: 1060px;
  margin: 0 auto;
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 20px;
  flex-wrap: wrap;
}
.head h1 {
  margin: 0;
  font-size: 22px;
  font-weight: 600;
}
.head p {
  margin: 5px 0 0;
  font-size: 12.5px;
}

.stats {
  display: flex;
  gap: 22px;
}
.stat {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}
.num {
  font-size: 16px;
  font-weight: 500;
}
.lab {
  font-size: 10.5px;
  color: var(--text-3);
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}
.tabs {
  display: flex;
  gap: 2px;
  padding: 3px;
  border-radius: var(--r-sm);
  background: var(--surface-2);
}
.tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 12px;
  border-radius: 7px;
  font-size: 12.5px;
  color: var(--text-2);
  transition: all var(--dur) var(--ease);
}
.tab.on {
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-sm);
  font-weight: 500;
}
.count {
  font-size: 10px;
  color: var(--text-3);
}

.list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.task {
  display: grid;
  grid-template-columns: 46px minmax(0, 1fr) auto;
  gap: 14px;
  align-items: center;
  padding: 12px 14px;
  border-radius: var(--r-md);
  border: 1px solid transparent;
  transition: background var(--dur) var(--ease), border-color var(--dur) var(--ease);
}
.task:hover {
  background: var(--surface);
  border-color: var(--line);
}

.cover {
  width: 46px;
  height: 46px;
  border-radius: 11px;
  overflow: hidden;
  background: var(--surface-2);
}
.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.body {
  min-width: 0;
}
.line {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.title {
  margin: 0;
  font-size: 13.5px;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.badge {
  font-size: 10px;
  padding: 1px 7px;
  border-radius: var(--r-full);
  background: var(--surface-2);
  color: var(--text-2);
  flex: 0 0 auto;
}
.badge.downloading,
.badge.resolving,
.badge.tagging {
  background: var(--active);
  color: var(--accent);
}
.badge.completed {
  background: color-mix(in srgb, var(--accent-2) 16%, transparent);
  color: var(--accent-2);
}
.badge.failed {
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  color: var(--danger);
}
.src {
  font-size: 10.5px;
  flex: 0 0 auto;
}
.sub {
  margin: 2px 0 7px;
  font-size: 11.5px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.dot {
  margin: 0 4px;
  opacity: 0.5;
}

.progress {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.track {
  height: 3px;
  border-radius: var(--r-full);
  background: var(--surface-3);
  overflow: hidden;
}
.fill {
  height: 100%;
  border-radius: var(--r-full);
  background: linear-gradient(90deg, var(--accent), var(--accent-soft));
  transition: width 320ms linear;
}
.progress.done .fill {
  background: var(--accent-2);
}
.progress-meta {
  font-size: 10.5px;
  color: var(--text-3);
}
.err {
  color: var(--danger);
}

.actions {
  display: flex;
  gap: 2px;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding: 72px 20px;
}
.empty-art {
  width: 62px;
  height: 62px;
  display: grid;
  place-items: center;
  border-radius: 20px;
  color: var(--accent);
  background: var(--active);
  margin-bottom: 16px;
}
.empty h3 {
  margin: 0 0 6px;
  font-size: 15px;
  font-weight: 500;
}
.empty p {
  margin: 0;
  font-size: 12.5px;
  max-width: 380px;
}

@media (max-width: 860px) {
  .view {
    padding: 18px 14px 26px;
  }
  .stats {
    gap: 16px;
  }
  .task {
    grid-template-columns: 40px minmax(0, 1fr);
    gap: 10px;
    padding: 10px;
    position: relative;
  }
  .cover {
    width: 40px;
    height: 40px;
  }
  /* 移动端把操作按钮收进行内右下角，避免单独占一行撑高卡片 */
  .actions {
    position: absolute;
    right: 6px;
    bottom: 6px;
  }
  .progress {
    padding-right: 64px;
  }
}
</style>
