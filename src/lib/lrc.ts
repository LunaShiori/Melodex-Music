export interface LrcLine {
  timeMs: number
  text: string
  translation?: string
}

function parseStamp(tag: string): number | null {
  const match = tag.match(/^(\d+):(\d+)(?:[.:](\d+))?$/)
  if (!match) return null
  const minutes = Number(match[1])
  const seconds = Number(match[2])
  const fraction = match[3] ?? '0'
  const millis = Number(fraction.padEnd(3, '0').slice(0, 3))
  return minutes * 60_000 + seconds * 1000 + millis
}

export function parseLrc(raw: string): LrcLine[] {
  const lines: LrcLine[] = []
  for (const original of raw.split(/\r?\n/)) {
    let rest = original.trim()
    if (!rest) continue
    const stamps: number[] = []
    while (rest.startsWith('[')) {
      const end = rest.indexOf(']')
      if (end < 0) break
      const stamp = parseStamp(rest.slice(1, end))
      rest = rest.slice(end + 1)
      if (stamp !== null) stamps.push(stamp)
    }
    const text = rest.trim()
    if (!stamps.length || !text) continue
    stamps.forEach((timeMs) => lines.push({ timeMs, text }))
  }
  lines.sort((a, b) => a.timeMs - b.timeMs)
  return lines
}

export function mergeTranslation(lines: LrcLine[], translation: string | null): LrcLine[] {
  if (!translation) return lines
  const translated = parseLrc(translation)
  return lines.map((line) => {
    const hit = translated.find((item) => Math.abs(item.timeMs - line.timeMs) < 50)
    return hit ? { ...line, translation: hit.text } : line
  })
}

export function activeLineIndex(lines: LrcLine[], positionMs: number): number {
  if (!lines.length) return -1
  let index = 0
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i].timeMs <= positionMs) index = i
    else break
  }
  return index
}
