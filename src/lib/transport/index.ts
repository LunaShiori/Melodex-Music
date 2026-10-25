import { createDemoTransport } from './demo'
import { createTauriTransport } from './tauri'
import type { Transport } from './types'

export type { SearchParams, Transport } from './types'

function hasTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/**
 * 有 Tauri 外壳时用 Rust 核心，否则退到内置演示数据。
 * 想强制走演示模式（例如设计走查）：`VITE_TRANSPORT=demo npm run dev`
 */
export const transport: Transport =
  import.meta.env.VITE_TRANSPORT === 'demo' || !hasTauri()
    ? createDemoTransport()
    : createTauriTransport()
