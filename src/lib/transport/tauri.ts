import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import type {
  AccountInfo,
  AggregatedPage,
  DownloadTask,
  EngineEvent,
  EnqueueInput,
  Lyric,
  PlayUrl,
  QrPoll,
  QrTicket,
  Settings,
  SourceId,
  UpdateInfo,
  VaultStatus,
  Variant,
} from '../types'
import type { SearchParams, Transport } from './types'

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

    resolve(variant: Variant, bitrate = 320) {
      return invoke<PlayUrl>('resolve', {
        source: variant.source,
        id: variant.id,
        bitrate,
      })
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
  }
}
