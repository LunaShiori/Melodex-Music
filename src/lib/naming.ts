import { sanitizeName } from './sanitize'

export interface NamingInput {
  title: string
  artists: string[]
  album: string
  source: string
  bitrate: number
  index?: number
  artistSeparator?: string
}

/** 与服务端一致的命名模板渲染，仅用于设置页实时预览。 */
export function renderNaming(template: string, input: NamingInput): string {
  const artists = input.artists.join(input.artistSeparator ?? ' / ')
  const out = template.replace(/\{(title|artist|album|source|bitrate|index(?::(\d+))?)\}/g, (
    match,
    key: string,
    pad?: string,
  ) => {
    switch (key.split(':')[0]) {
      case 'title':
        return input.title
      case 'artist':
        return artists
      case 'album':
        return input.album
      case 'source':
        return input.source
      case 'bitrate':
        return String(input.bitrate)
      case 'index':
        return String(input.index ?? 1).padStart(Number(pad ?? 0) || 0, '0')
      default:
        return match
    }
  })
  return sanitizeName(out)
}
