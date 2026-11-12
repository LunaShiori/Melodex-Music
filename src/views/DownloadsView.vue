<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import Icon from '@/components/Icon.vue'
import { coverOf } from '@/lib/artwork'
import { sourceShort, useI18n } from '@/lib/i18n'
import { onCoverError } from '@/lib/cover'
import { formatBytes, formatRelative, formatSpeed } from '@/lib/format'
import { reasonMeta } from '@/lib/reason'
import { SOURCE_META } from '@/lib/types'
import type { DownloadTask, TaskState } from '@/lib/types'
import { useTasksStore } from '@/stores/tasks'

const { t } = useI18n()
const tasks = useTasksStore()
const router = useRouter()

const STATE_KEY: Record<TaskState, string> = {
  queued: 'state.queued',
  resolving: 'state.resolving',
  downloading: 'state.downloading',
  tagging: 'state.tagging',
  completed: 'state.completed',
  failed: 'state.failed',
  paused: 'state.paused',
  canceled: 'state.canceled',
}
const stateText = (state: TaskState) => t(STATE_KEY[state])

const tabs = computed(() => [
  { key: 'all' as const, label: t('downloads.tabAll'), count: tasks.counts.all },
  { key: 'active' as const, label: t('downloads.tabActive'), count: tasks.counts.active },
  { key: 'done' as const, label: t('downloads.tabDone'), count: tasks.counts.done },
])

function ratio(task: DownloadTask) {
  if (!task.total) return task.state === 'tagging' ? 0.99 : 0.08
  return Math.min(1, task.downloaded / task.total)
}

function isActive(task: DownloadTask) {
  return ['queued', 'resolving', 'downloading', 'tagging', 'paused'].includes(task.state)
}

/** 失败条目优先展示结构化原因，取不到时回落到引擎给的原文。 */
function failText(task: DownloadTask) {
  return reasonMeta(task.failReason)?.hint ?? task.error
}

/** 只有「登录能解决」的原因才给登录入口，避免误导。 */
function needsLogin(task: DownloadTask) {
  return task.state === 'failed' && !!reasonMeta(task.failReason)?.login
}

function goLogin() {
  void router.push({ name: 'settings' })
}
</script>

<template>
  <div class="view">
    <header class="head">
      <div>
        <h1>{{ t('downloads.title') }}</h1>
        <p class="muted">{{ t('downloads.subtitle') }}</p>
      </div>
      <div class="stats">
        <div class="stat">
          <span class="num mono">{{ tasks.counts.active }}</span>
          <span class="lab">{{ t('downloads.inProgress') }}</span>
        </div>
        <div class="stat">
          <span class="num mono">{{ tasks.counts.done }}</span>
          <span class="lab">{{ t('downloads.finished') }}</span>
        </div>
        <div class="stat">
          <span class="num mono">{{ formatSpeed(tasks.totalSpeed) }}</span>
          <span class="lab">{{ t('downloads.totalSpeed') }}</span>
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
        {{ t('downloads.clearDone') }}
      </button>
    </div>

    <section v-if="tasks.visible.length" class="list">
      <article v-for="task in tasks.visible" :key="task.id" class="task" :class="task.state">
        <div class="cover">
          <img :src="coverOf(task.cover, task.id)" alt="" loading="lazy" @error="onCoverError($event, task.id)" />
        </div>

        <div class="body">
          <div class="line">
            <p class="title">{{ task.title }}</p>
            <span class="badge" :class="task.state">{{ stateText(task.state) }}</span>
            <span
              v-if="task.state === 'failed' && reasonMeta(task.failReason)"
              class="badge reason"
            >
              {{ reasonMeta(task.failReason)?.label }}
            </span>
            <span
              class="src"
              :style="{ color: SOURCE_META[task.resolvedSource ?? task.source].color }"
            >
              {{ sourceShort(task.resolvedSource ?? task.source) }}
            </span>
            <span
              v-if="task.resolvedSource && task.resolvedSource !== task.source"
              class="badge swapped"
              title="{{ t('downloads.switchedSource') }}"
            >
              {{ t('downloads.autoSource') }}
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
                {{ formatBytes(task.total || task.downloaded) }} · {{ t('downloads.saved') }}
              </span>
              <span v-else-if="task.state === 'failed'" class="err">{{ failText(task) }}</span>
              <span v-else>{{ stateText(task.state) }}</span>
              <!-- 登录提示紧跟在失败原因后面，不再塞进右侧操作区：
                   手机上操作区是绝对定位的，长文案会被按钮压住。 -->
              <button
                v-if="task.state === 'failed' && needsLogin(task)"
                class="btn btn-sm login-cta"
                type="button"
                title="{{ t('downloads.loginSource') }}"
                @click="goLogin()"
              >
                <Icon name="user" :size="12" />
                {{ t('downloads.goLogin') }}
              </button>
            </div>
          </div>
        </div>

        <div class="actions">
          <button
            v-if="!isActive(task) && task.state !== 'completed'"
            class="icon-btn"
            title="{{ t('downloads.retry') }}"
            type="button"
            @click="tasks.resume(task.id)"
          >
            <Icon name="refresh" :size="16" />
          </button>
          <button
            v-if="task.state === 'downloading' || task.state === 'queued'"
            class="icon-btn"
            title="{{ t('downloads.pause') }}"
            type="button"
            @click="tasks.pause(task.id)"
          >
            <Icon name="pause" :size="16" />
          </button>
          <button
            v-if="task.state === 'paused'"
            class="icon-btn"
            title="{{ t('downloads.resume') }}"
            type="button"
            @click="tasks.resume(task.id)"
          >
            <Icon name="play" :size="15" />
          </button>
          <button
            v-if="task.state === 'completed' && task.path"
            class="icon-btn"
            title="{{ t('downloads.reveal') }}"
            type="button"
            @click="tasks.reveal(task.path!)"
          >
            <Icon name="folder" :size="16" />
          </button>
          <button class="icon-btn" title="{{ t('downloads.remove') }}" type="button" @click="tasks.remove(task.id)">
            <Icon name="close" :size="16" />
          </button>
        </div>
      </article>
    </section>

    <section v-else class="empty">
      <div class="empty-art"><Icon name="download" :size="26" /></div>
      <h3>{{ t('downloads.emptyTitle') }}</h3>
      <p class="muted">{{ t('downloads.emptyDesc') }}</p>
      <RouterLink to="/search" class="btn btn-primary" style="margin-top: 16px">{{ t('downloads.goSearch') }}</RouterLink>
    </section>
  </div>
</template>

<style scoped>
.view {
  padding: var(--view-pad);
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
/* 失败原因：和「失败」状态区分开，让用户一眼看出是哪种不可用 */
.badge.reason {
  background: color-mix(in srgb, var(--warning) 16%, transparent);
  color: var(--warning);
}
.badge.swapped {
  background: color-mix(in srgb, var(--accent-2) 14%, transparent);
  color: var(--accent-2);
}
.login-cta {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
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
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
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
    padding: calc(env(safe-area-inset-top, 0px) + 18px) 14px
      calc(env(safe-area-inset-bottom, 0px) + 26px);
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
