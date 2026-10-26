import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import type {
  AccountInfo,
  AggregatedPage,
  DownloadTask,
  EngineEvent,
  EnqueueInput,
  Lyric,
  PlatformQuality,
  PlayUrl,
  QrPoll,
  QrTicket,
  QualityLevel,
  Settings,
  SourceId,
  UpdateInfo,
  UpdateProgress,
  VaultStatus,
  Variant,
} from '../types'
import type { LoginWebEvent, SearchParams, Transport } from './types'

/** Tauri 外壳：所有能力都由 Rust 核心通过 command 暴露。 */
export function createTauriTransport(): Transport {
  return {
    mode: 'tauri',

    search(params: SearchParams) {
      return invoke<AggregatedPage>('search', {
        keyword: params.keyword,
        page: params.page,
        limit: params.limit,
        sources: params.sources,
      })
    },

    resolve(variant: Variant, quality?: QualityLevel) {
      return invoke<PlayUrl>('resolve', {
        source: variant.source,
        id: variant.id,
        quality: quality ?? null,
      })
    },

    platformQualities() {
      return invoke<PlatformQuality[]>('platform_qualities')
    },

    lyric(ref: { source: SourceId; id: string }) {
      return invoke<Lyric | null>('lyric', { source: ref.source, id: ref.id })
    },

    enqueue(inputs: EnqueueInput[]) {
      return invoke<string[]>('enqueue', { inputs })
    },

    tasks() {
      return invoke<DownloadTask[]>('list_tasks')
    },

    async pause(id: string) {
      await invoke('pause_task', { id })
    },

    async resume(id: string) {
      await invoke('resume_task', { id })
    },

    async remove(id: string) {
      await invoke('remove_task', { id })
    },

    clearFinished() {
      return invoke<number>('clear_finished')
    },

    getSettings() {
      return invoke<Settings>('get_settings')
    },

    async saveSettings(settings: Settings) {
      await invoke('save_settings', { settings })
    },

    onEvent(handler: (event: EngineEvent) => void) {
      let disposed = false
      let unlisten: (() => void) | null = null
      void listen<EngineEvent>('melodex://event', (event) => handler(event.payload)).then((off) => {
        if (disposed) {
          off()
        } else {
          unlisten = off
        }
      })
      return () => {
        disposed = true
        unlisten?.()
      }
    },

    defaultDownloadDir() {
      return invoke<string>('default_download_dir')
    },

    async reveal(path: string) {
      await invoke('reveal_path', { path })
    },

    checkUpdate(feed?: string) {
      return invoke<UpdateInfo>('check_update', { feed: feed ?? null })
    },

    updateDownload(info: UpdateInfo, assetName?: string) {
      return invoke<string>('update_download', { info, assetName: assetName ?? null })
    },

    async updateInstall(path: string) {
      await invoke('update_install', { path })
    },

    onUpdateProgress(handler) {
      let disposed = false
      let unlisten: (() => void) | null = null
      void listen<UpdateProgress>('update-progress', (event) => handler(event.payload)).then(
        (off) => {
          if (disposed) {
            off()
          } else {
            unlisten = off
          }
        },
      )
      return () => {
        disposed = true
        unlisten?.()
      }
    },

    async openUrl(url: string) {
      await invoke('open_url', { url })
    },

    localUrl(path: string) {
      return convertFileSrc(path)
    },

    vaultStatus() {
      return invoke<VaultStatus>('vault_status')
    },

    vaultUnlock(pin: string) {
      return invoke<VaultStatus>('vault_unlock', { pin })
    },

    vaultSetPin(pin: string | null) {
      return invoke<VaultStatus>('vault_set_pin', { pin })
    },

    loginQrStart(source: SourceId) {
      return invoke<QrTicket>('login_qr_start', { source })
    },

    loginQrPoll(source: SourceId, key: string) {
      return invoke<QrPoll>('login_qr_poll', { source, key })
    },

    saveAccount(source: SourceId, cookie: string, nickname?: string | null) {
      return invoke<AccountInfo>('account_save', {
        source,
        cookie,
        nickname: nickname ?? null,
      })
    },

    clearAccount(source: SourceId) {
      return invoke<VaultStatus>('account_clear', { source })
    },

    loginWebSupported() {
      return invoke<boolean>('login_web_supported')
    },

    async loginWebStart(source: SourceId) {
      await invoke('login_web_start', { source })
    },

    onLoginWeb(handler: (event: LoginWebEvent) => void) {
      let disposed = false
      const disposers: Array<() => void> = []
      const bind = <T>(name: string, map: (payload: T) => LoginWebEvent) => {
        void listen<T>(name, (event) => handler(map(event.payload))).then((off) => {
          if (disposed) {
            off()
          } else {
            disposers.push(off)
          }
        })
      }
      // 后端按结果分别 emit 三个事件，这里统一成一个回调
      bind<AccountInfo>('login-web-done', (account) => ({ type: 'done', account }))
      bind<string>('login-web-error', (message) => ({ type: 'error', message }))
      bind<string>('login-web-closed', () => ({ type: 'closed' }))
      return () => {
        disposed = true
        for (const off of disposers.splice(0)) off()
      }
    },
  }
}
