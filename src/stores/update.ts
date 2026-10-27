import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { transport } from '@/lib/transport'
import type { UpdateInfo } from '@/lib/types'

/** 在线更新走到哪一步 */
export type UpdateStage = 'idle' | 'downloading' | 'ready' | 'installing'

/** 安卓的安装是「拉起系统安装器」而不是「装完自动重启」，界面上要区别对待 */
export const IS_ANDROID = /Android/i.test(navigator.userAgent)

/**
 * 在线更新。
 *
 * 默认路径是「应用内下载 → 拉起安装器 → 自动重启」，用户不需要离开应用；
 * 打开发布页只是备用入口（想自己挑安装包、或自动安装失败时用）。
 */
export const useUpdateStore = defineStore('update', () => {
  const info = ref<UpdateInfo | null>(null)
  const checking = ref(false)
  const error = ref('')
  const dismissed = ref(false)
  const checkedAt = ref<number | null>(null)

  const stage = ref<UpdateStage>('idle')
  const downloaded = ref(0)
  const totalBytes = ref(0)
  /** 下载完成后的本地路径 */
  const readyPath = ref('')
  /** 安装失败的原因（与检查更新的 `error` 分开，避免互相覆盖） */
  const installError = ref('')

  /** 是否需要在界面上提醒（有新版且用户没有关掉） */
  const showBanner = computed(() => !!info.value?.available && !dismissed.value)

  const percent = computed(() => {
    if (!totalBytes.value) return 0
    return Math.min(100, Math.round((downloaded.value / totalBytes.value) * 100))
  })

  const busy = computed(() => stage.value === 'downloading' || stage.value === 'installing')

  let unsubscribe: (() => void) | null = null
  function watchProgress() {
    if (unsubscribe) return
    unsubscribe = transport.onUpdateProgress((progress) => {
      downloaded.value = progress.downloaded
      if (progress.total) totalBytes.value = progress.total
    })
  }

  async function check(options: { silent?: boolean } = {}) {
    if (checking.value) return
    checking.value = true
    error.value = ''
    try {
      const previous = info.value?.latest
      info.value = await transport.checkUpdate()
      dismissed.value = false
      checkedAt.value = Date.now()
      // 版本变了就重置下载状态，避免把上一个版本的进度带过来
      if (previous && previous !== info.value.latest) {
        reset()
      }
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
      if (!options.silent) throw err
    } finally {
      checking.value = false
    }
  }

  /** 下载安装包。`assetName` 留空时由内核按当前平台挑。 */
  async function download(assetName?: string) {
    if (!info.value || busy.value) return
    watchProgress()
    stage.value = 'downloading'
    installError.value = ''
    downloaded.value = 0
    totalBytes.value = 0
    try {
      readyPath.value = await transport.updateDownload(info.value, assetName)
      stage.value = 'ready'
    } catch (err) {
      installError.value = err instanceof Error ? err.message : String(err)
      stage.value = 'idle'
    }
  }

  /** 拉起安装包。Windows 上应用会随即退出，由安装器接管。 */
  async function install() {
    if (!readyPath.value || busy.value) return
    stage.value = 'installing'
    installError.value = ''
    try {
      await transport.updateInstall(readyPath.value)
      // Windows 上进程马上就要退出，这句不会执行到；安卓只是把系统安装器叫起来，
      // 用户随时可能返回应用——状态必须放回 ready，否则会一直卡在「正在退出…」
      // 且所有按钮都被 disabled。
      if (IS_ANDROID) stage.value = 'ready'
    } catch (err) {
      installError.value = err instanceof Error ? err.message : String(err)
      stage.value = 'ready'
    }
  }

  function reset() {
    stage.value = 'idle'
    downloaded.value = 0
    totalBytes.value = 0
    readyPath.value = ''
    installError.value = ''
  }

  function dismiss() {
    dismissed.value = true
  }

  return {
    info,
    checking,
    error,
    dismissed,
    checkedAt,
    showBanner,
    stage,
    downloaded,
    totalBytes,
    readyPath,
    installError,
    percent,
    busy,
    check,
    download,
    install,
    reset,
    dismiss,
  }
})
