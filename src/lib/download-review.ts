import type { DownloadTask, Track } from './types'
/** Match exact source IDs, never titles or album names. */
export function taskForTrack(track: Track, tasks: DownloadTask[]): DownloadTask | undefined {
  return tasks.filter(task => track.variants.some(v => v.source === task.source && v.id === task.sourceId))
    .sort((a,b) => b.createdAt - a.createdAt)[0]
}
export function reviewTracks(tracks: Track[], tasks: DownloadTask[]) {
  const rows = tracks.map(track => ({track, task: taskForTrack(track, tasks)}))
  return { rows, completed: rows.filter(r=>r.task?.state==='completed').length,
    failed: rows.filter(r=>r.task?.state==='failed'),
    missing: rows.filter(r=>!r.task || r.task.state==='canceled'),
    active: rows.filter(r=>r.task && !['completed','failed','canceled'].includes(r.task.state)).length }
}
export function deliveredAudio(task: DownloadTask): string | null {
  if (!['downloading','tagging','completed'].includes(task.state)) return null
  return [task.format?.toUpperCase(), task.bitrate > 0 ? task.bitrate + ' kbps' : ''].filter(Boolean).join(' · ') || null
}

/** Only flag a downgrade when returned format/bitrate proves it; bitrate cannot prove Hi-Res. */
export function knownDowngrade(task: DownloadTask): boolean {
  if (!deliveredAudio(task)) return false
  const lossy = ['mp3','aac','ogg','opus'].includes(task.format.toLowerCase())
  if (task.quality === 'lossless' || task.quality === 'hiRes' || task.quality === 'master') return lossy
  const expected = {standard:128,high:192,exquisite:320}[task.quality]
  return lossy && task.bitrate > 0 && task.bitrate < expected
}
