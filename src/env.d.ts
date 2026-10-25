/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<Record<string, unknown>, Record<string, unknown>, unknown>
  export default component
}

/** 由 vite.config.ts 注入的版本号（来源 package.json） */
declare const __APP_VERSION__: string
