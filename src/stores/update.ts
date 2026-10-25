import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { transport } from '@/lib/transport'
import type { UpdateInfo } from '@/lib/types'

/**
 * 在线更新：只负责「检查 + 提示」。
 * 不自动下载、不静默替换文件——安装包由用户自己点开，行为可预期。
 */
export const useUpdateStore = defineStore('update', () => {
  const info = ref<UpdateInfo | null>(null)
  const checking = ref(false)
  const error = ref('')
  const dismissed = ref(false)
  const checkedAt = ref<number | null>(null)

  /** 是否需要在界面上提醒（有新版且用户没有关掉） */
  const showBanner = computed(() => !!info.value?.available && !dismissed.value)

  async function check(options: { silent?: boolean } = {}) {
    if (checking.value) return
    checking.value = true
    error.value = ''
    try {
      info.value = await transport.checkUpdate()
      dismissed.value = false
      checkedAt.value = Date.now()
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
      if (!options.silent) throw err
    } finally {
      checking.value = false
    }
  }

  function dismiss() {
    dismissed.value = true
  }

  return { info, checking, error, dismissed, checkedAt, showBanner, check, dismiss }
})
