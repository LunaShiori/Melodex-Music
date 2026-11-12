import { computed, ref, watch } from 'vue'

import { en } from '@/i18n/en'
import { zh } from '@/i18n/zh'

export type Locale = 'zh' | 'en'
/** auto = 跟随系统语言，实际解析结果见 resolvedLocale */
export type LocaleChoice = Locale | 'auto'

const STORE_KEY = 'melodex.locale'

const dicts: Record<Locale, Record<string, string>> = { zh, en }

function detect(): Locale {
  if (typeof navigator === 'undefined') return 'zh'
  const lang = (navigator.language || '').toLowerCase()
  return lang.startsWith('zh') ? 'zh' : 'en'
}

function readChoice(): LocaleChoice {
  if (typeof localStorage === 'undefined') return 'auto'
  try {
    const raw = localStorage.getItem(STORE_KEY)
    if (raw === 'zh' || raw === 'en' || raw === 'auto') return raw
  } catch {
    // 隐私模式下读不到就按跟随系统处理
  }
  return 'auto'
}

/** 用户的选择（可持久化），实际生效语言由 resolvedLocale 给出 */
export const localeChoice = ref<LocaleChoice>(readChoice())
export const systemLocale = ref<Locale>(detect())

export const resolvedLocale = computed<Locale>(() =>
  localeChoice.value === 'auto' ? systemLocale.value : localeChoice.value,
)

export function setLocale(next: LocaleChoice) {
  localeChoice.value = next
  try {
    localStorage.setItem(STORE_KEY, next)
  } catch {
    // 存不下时只影响下次启动的默认值
  }
}

// 让 <html lang> 跟着语言走：读屏软件据此选发音，浏览器也会换字体
if (typeof document !== 'undefined') {
  watch(
    resolvedLocale,
    (next) => {
      document.documentElement.lang = next === 'zh' ? 'zh-CN' : 'en'
    },
    { immediate: true },
  )
}

/**
 * 取词条。`key` 用点号分级，`{name}` 占位符按 params 替换；
 * 缺词条时回落到中文，再缺就原样返回 key（便于发现漏翻）。
 */
export function t(key: string, params?: Record<string, string | number>): string {
  const locale = resolvedLocale.value
  const template = dicts[locale][key] ?? dicts.zh[key] ?? key
  if (!params) return template
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    Object.prototype.hasOwnProperty.call(params, name) ? String(params[name]) : match,
  )
}

/** 歌手名单：空列表时显示「未知歌手 / Unknown artist」，避免每处各写一遍 */
export function artistName(artists: readonly string[] | undefined): string {
  const joined = (artists ?? []).join(' / ')
  return joined || t('common.unknownArtist')
}

/** 音源显示名，跟着界面语言走 */
export function sourceShort(id: string): string {
  return t(`source.${id}.short`)
}

export function sourceName(id: string): string {
  return t(`source.${id}`)
}

/** 组件里用：`const { t, locale, setLocale } = useI18n()` */
export function useI18n() {
  return { t, locale: resolvedLocale, choice: localeChoice, setLocale }
}
