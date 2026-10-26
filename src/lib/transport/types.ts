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

export interface SearchParams {
  keyword: string
  page: number
  limit: number
  sources: SourceId[]
}

/** 内置网页登录的推进事件 */
export type LoginWebEvent =
  | { type: 'done'; account: AccountInfo }
  | { type: 'error'; message: string }
  /** 用户自己关掉了登录窗口 */
  | { type: 'closed' }

/**
 * 平台传输层：界面只依赖这个接口。
 * - Tauri 端实现直接 invoke Rust 核心；
 * - 演示端（浏览器 / 预览）用本地模拟数据，方便设计走查。
 */
export interface Transport {
  readonly mode: 'tauri' | 'demo'
  search(params: SearchParams): Promise<AggregatedPage>
  /** 解析试听链接。`quality` 省略时用设置里该音源的目标音质 */
  resolve(variant: Variant, quality?: QualityLevel): Promise<PlayUrl>
  /** 各音源的音质档位表（名称与规格由后端唯一维护） */
  platformQualities(): Promise<PlatformQuality[]>
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
  /** 桌面端弹目录选择框，返回选中的目录（取消为 null） */
  pickDownloadDir(): Promise<string | null>
  reveal(path: string): Promise<void>
  /** 检查线上是否有新版本（feed 留空用内置发布源） */
  checkUpdate(feed?: string): Promise<UpdateInfo>
  /**
   * 下载安装包到本地缓存，返回绝对路径。
   * 不传 `assetName` 时由内核按当前平台挑最合适的那个。
   */
  updateDownload(info: UpdateInfo, assetName?: string): Promise<string>
  /** 拉起已下载的安装包：Windows 运行安装器并退出应用，Android 调系统安装器 */
  updateInstall(path: string): Promise<void>
  /** 订阅下载进度，返回取消订阅函数 */
  onUpdateProgress(handler: (progress: UpdateProgress) => void): () => void
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
  /** 当前平台是否支持内置网页登录（Android 的 WebView 读不到 Cookie，只能扫码） */
  loginWebSupported(): Promise<boolean>
  /** 打开内置登录窗口；结果通过 `onLoginWeb` 推送，不在这里返回 */
  loginWebStart(source: SourceId): Promise<void>
  /** 订阅网页登录结果，返回取消订阅函数 */
  onLoginWeb(handler: (event: LoginWebEvent) => void): () => void
}
