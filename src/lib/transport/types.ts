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

export interface SearchParams {
  keyword: string
  page: number
  limit: number
  sources: SourceId[]
}

/**
 * 平台传输层：界面只依赖这个接口。
 * - Tauri 端实现直接 invoke Rust 核心；
 * - 演示端（浏览器 / 预览）用本地模拟数据，方便设计走查。
 */
export interface Transport {
  readonly mode: 'tauri' | 'demo'
  search(params: SearchParams): Promise<AggregatedPage>
  resolve(variant: Variant, bitrate?: number): Promise<PlayUrl>
  lyric(ref: { source: SourceId; id: string }): Promise<Lyric | null>
  enqueue(inputs: EnqueueInput[]): Promise<string[]>
  tasks(): Promise<DownloadTask[]>
  pause(id: string): Promise<void>
  resume(id: string): Promise<void>
  remove(id: string): Promise<void>
  clearFinished(): Promise<number>
  getSettings(): Promise<Settings>
  saveSettings(settings: Settings): Promise<void>
  onEvent(handler: (event: EngineEvent) => void): () => void
  defaultDownloadDir(): Promise<string>
  reveal(path: string): Promise<void>
  /** 检查线上是否有新版本（feed 留空用内置发布源） */
  checkUpdate(feed?: string): Promise<UpdateInfo>
  /** 用系统默认浏览器/应用打开外部链接 */
  openUrl(url: string): Promise<void>
  /** 把本地文件路径转成播放器可用的地址 */
  localUrl(path: string): string

  /* ---- 账号与本地加密凭据 ---- */
  vaultStatus(): Promise<VaultStatus>
  /** 用 PIN 解锁凭据库 */
  vaultUnlock(pin: string): Promise<VaultStatus>
  /** 设置 / 取消 PIN（传 null 取消） */
  vaultSetPin(pin: string | null): Promise<VaultStatus>
  /** 申请扫码登录二维码（网易云） */
  loginQrStart(source: SourceId): Promise<QrTicket>
  /** 轮询扫码状态；确认成功后后端会直接把凭据写入加密库 */
  loginQrPoll(source: SourceId, key: string): Promise<QrPoll>
  /** 手动保存 Cookie（QQ 音乐等） */
  saveAccount(source: SourceId, cookie: string, nickname?: string | null): Promise<AccountInfo>
  /** 退出登录 */
  clearAccount(source: SourceId): Promise<VaultStatus>
}
