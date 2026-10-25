/** 与 Rust 端 sanitize 保持一致的简易实现（预览用）。 */
export function sanitizeName(raw: string): string {
  const cleaned = raw.replace(/[<>:"/\\|?*\u0000-\u001f]/g, ' ').replace(/\s+/g, ' ').trim()
  const trimmed = cleaned.replace(/^[.\s]+|[.\s]+$/g, '')
  const limited = [...(trimmed || '未命名')].slice(0, 120).join('')
  const reserved = ['CON', 'PRN', 'AUX', 'NUL', 'COM1', 'COM2', 'COM3', 'COM4', 'LPT1']
  return reserved.some((name) => name.toLowerCase() === limited.toLowerCase())
    ? `_${limited}`
    : limited
}
