import type { FailReason } from './types'

export interface ReasonMeta {
  /** 短标签，跟在歌名后面 */
  label: string
  /** 一句话解释，鼠标悬停或展开时显示 */
  hint: string
  /** 是否该引导用户去登录 */
  login: boolean
  /** 是否值得再点一次重试 */
  retry: boolean
}

/**
 * 与核心层 `error::Reason` 一一对应。
 *
 * 这里的措辞直接面向用户：说清「为什么下不了」，而不是抛一个技术错。
 */
export const REASON_META: Record<FailReason, ReasonMeta> = {
  needLogin: {
    label: 'reason.needLogin.label',
    hint: 'reason.needLogin.hint',
    login: true,
    retry: false,
  },
  vipOnly: {
    label: 'reason.vipOnly.label',
    hint: 'reason.vipOnly.hint',
    login: true,
    retry: false,
  },
  qualityMissing: {
    label: 'reason.qualityMissing.label',
    hint: 'reason.qualityMissing.hint',
    login: false,
    retry: false,
  },
  copyright: {
    label: 'reason.copyright.label',
    hint: 'reason.copyright.hint',
    login: false,
    retry: false,
  },
  network: {
    label: 'reason.network.label',
    hint: 'reason.network.hint',
    login: false,
    retry: true,
  },
  linkRejected: {
    label: 'reason.linkRejected.label',
    hint: 'reason.linkRejected.hint',
    login: false,
    retry: true,
  },
  invalid: {
    label: 'reason.invalid.label',
    hint: 'reason.invalid.hint',
    login: false,
    retry: false,
  },
  unknown: {
    label: 'reason.unknown.label',
    hint: 'reason.unknown.hint',
    login: false,
    retry: true,
  },
}

/** 取不到原因时返回 `null`，调用方回落到原始 `error` 文案。 */
export function reasonMeta(reason: FailReason | null | undefined): ReasonMeta | null {
  if (!reason) return null
  return REASON_META[reason] ?? null
}
