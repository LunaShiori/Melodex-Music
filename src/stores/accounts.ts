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
    if (account) await refresh()
    return account
  }

  function startQr(source: SourceId): Promise<QrTicket> {
    return transport.loginQrStart(source)
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
    refresh,
    unlock,
    setPin,
    clear,
    saveCookie,
    startQr,
    pollQr,
  }
})
