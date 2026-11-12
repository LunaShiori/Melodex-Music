import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { transport } from '@/lib/transport'
import type { AccountInfo, QrPoll, QrTicket, SourceId, VaultStatus } from '@/lib/types'

/**
 * 账号状态：凭据只存在 Rust 侧的加密保险库里，这里只持有「谁登录了」这类摘要，
 * 界面任何地方都拿不到 Cookie 明文。
 */
export const useAccountsStore = defineStore('accounts', () => {
  const status = ref<VaultStatus | null>(null)
  const loading = ref(false)
  const error = ref('')

  /** source -> 账号摘要 */
  const bySource = computed<Partial<Record<SourceId, AccountInfo>>>(() => {
    const map: Partial<Record<SourceId, AccountInfo>> = {}
    for (const account of status.value?.accounts ?? []) {
      map[account.source] = account
    }
    return map
  })

  const needsUnlock = computed(
    () => Boolean(status.value?.pinProtected) && !status.value?.unlocked,
  )

  function accountOf(source: SourceId): AccountInfo | null {
    return bySource.value[source] ?? null
  }

  /**
   * 凭据过期标记。
   *
   * 我们不主动轮询校验 Cookie（每个源都打一次接口太吵），改成「用的时候发现」：
   * 下载或取链报 needLogin，但保险库里明明有该源账号，就说明凭据过期了。
   * 标记存在本地，下次打开也还在，直到用户重新登录或主动清除。
   */
  const STALE_KEY = 'melodex.stale.v1'
  const stale = ref<SourceId[]>(readStale())

  function readStale(): SourceId[] {
    if (typeof localStorage === 'undefined') return []
    try {
      const raw = JSON.parse(localStorage.getItem(STALE_KEY) ?? '[]') as unknown
      return Array.isArray(raw) ? (raw.filter((item) => typeof item === 'string') as SourceId[]) : []
    } catch {
      return []
    }
  }

  function persistStale() {
    if (typeof localStorage === 'undefined') return
    try {
      localStorage.setItem(STALE_KEY, JSON.stringify(stale.value))
    } catch {
      /* 存不下就只在本次会话里提示 */
    }
  }

  function markStale(source: SourceId) {
    if (!accountOf(source) || stale.value.includes(source)) return
    stale.value = [...stale.value, source]
    persistStale()
  }

  function clearStale(source: SourceId) {
    if (!stale.value.includes(source)) return
    stale.value = stale.value.filter((item) => item !== source)
    persistStale()
  }

  /** 重新从本地读一次过期标记：别的标签页或调试时改过 localStorage，界面要跟得上 */
  function syncStale() {
    stale.value = readStale()
  }

  function isStale(source: SourceId) {
    return stale.value.includes(source)
  }

  async function run<T>(task: () => Promise<T>): Promise<T | null> {
    loading.value = true
    error.value = ''
    try {
      return await task()
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
      return null
    } finally {
      loading.value = false
    }
  }

  async function refresh() {
    const next = await run(() => transport.vaultStatus())
    if (next) status.value = next
  }

  async function unlock(pin: string) {
    const next = await run(() => transport.vaultUnlock(pin))
    if (next) status.value = next
    return Boolean(next)
  }

  async function setPin(pin: string | null) {
    const next = await run(() => transport.vaultSetPin(pin))
    if (next) status.value = next
    return Boolean(next)
  }

  async function clear(source: SourceId) {
    const next = await run(() => transport.clearAccount(source))
    if (next) status.value = next
  }

  async function saveCookie(source: SourceId, cookie: string, nickname?: string | null) {
    const account = await run(() => transport.saveAccount(source, cookie, nickname))
    if (account) {
      // 刚换过凭据，之前的过期标记作废
      clearStale(source)
      await refresh()
    }
    return account
  }

  function startQr(source: SourceId): Promise<QrTicket> {
    return transport.loginQrStart(source)
  }

  /** 打开内置登录窗口。返回是否成功打开（结果通过 onLoginWeb 事件推送） */
  async function startWebLogin(source: SourceId) {
    error.value = ''
    try {
      await transport.loginWebStart(source)
      return true
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
      return false
    }
  }

  async function pollQr(source: SourceId, key: string): Promise<QrPoll> {
    const poll = await transport.loginQrPoll(source, key)
    if (poll.status === 'confirmed') await refresh()
    return poll
  }

  return {
    status,
    loading,
    error,
    bySource,
    needsUnlock,
    accountOf,
    stale,
    markStale,
    syncStale,
    clearStale,
    isStale,
    refresh,
    unlock,
    setPin,
    clear,
    saveCookie,
    startQr,
    startWebLogin,
    pollQr,
  }
})
