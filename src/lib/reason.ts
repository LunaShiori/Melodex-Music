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
    label: '需要登录',
    hint: '该音源必须登录后才提供下载链接，登录后重试即可',
    login: true,
    retry: false,
  },
  vipOnly: {
    label: '会员专享',
    hint: '这首歌在该音源属于会员内容；可以换一个音源试试',
    login: true,
    retry: false,
  },
  qualityMissing: {
    label: '无此音质',
    hint: '该音源没有你要的音质，已自动尝试更低音质，仍不可用',
    login: false,
    retry: false,
  },
  copyright: {
    label: '已下架',
    hint: '版权方下架或当前地区不可播，换一个音源可能还有',
    login: false,
    retry: false,
  },
  network: {
    label: '网络异常',
    hint: '网络或音源服务暂时异常，稍后重试',
    login: false,
    retry: true,
  },
  linkRejected: {
    label: '直链被拒',
    hint: '音源给了播放地址，但它自己的 CDN 拒绝提供音频（多因链接过期或该地区节点不可用）',
    login: false,
    retry: true,
  },
  invalid: {
    label: '参数有误',
    hint: '请求参数不合法，换一首歌或重启应用',
    login: false,
    retry: false,
  },
  unknown: {
    label: '暂不可用',
    hint: '音源没有返回可用链接，稍后重试或换一个音源',
    login: false,
    retry: true,
  },
}

/** 取不到原因时返回 `null`，调用方回落到原始 `error` 文案。 */
export function reasonMeta(reason: FailReason | null | undefined): ReasonMeta | null {
  if (!reason) return null
  return REASON_META[reason] ?? null
}
