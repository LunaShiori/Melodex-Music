<script setup lang="ts">
/** 极简线性图标，统一 24 网格、1.6 描边，避免引入图标库。 */
const props = withDefaults(
  defineProps<{ name: string; size?: number; filled?: boolean }>(),
  { size: 18, filled: false },
)

const PATHS: Record<string, string[]> = {
  search: ['M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14Z', 'M16.5 16.5 21 21'],
  download: ['M12 3v11', 'M7.5 10 12 14.5 16.5 10', 'M4 20h16'],
  library: ['M4 6h10', 'M4 12h10', 'M4 18h6', 'M18 6v12', 'M15 15l3 3 3-3'],
  settings: ['M4 7h10', 'M18 7h2', 'M4 17h4', 'M12 17h8', 'M16 4v6', 'M8 14v6'],
  play: ['M8 5.5v13l11-6.5-11-6.5Z'],
  pause: ['M9 5v14', 'M15 5v14'],
  next: ['M6 5.5v13l10-6.5-10-6.5Z', 'M18 5v14'],
  prev: ['M18 5.5v13l-10-6.5 10-6.5Z', 'M6 5v14'],
  volume: ['M4 10v4h3l4 4V6L7 10H4Z', 'M15.5 9a4 4 0 0 1 0 6', 'M18 6.5a7.5 7.5 0 0 1 0 11'],
  mute: ['M4 10v4h3l4 4V6L7 10H4Z', 'M16 10l5 5', 'M21 10l-5 5'],
  sun: ['M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z', 'M12 2v2', 'M12 20v2', 'M2 12h2', 'M20 12h2', 'M5 5l1.5 1.5', 'M17.5 17.5 19 19', 'M19 5l-1.5 1.5', 'M6.5 17.5 5 19'],
  moon: ['M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5Z'],
  trash: ['M4 7h16', 'M9 7V4h6v3', 'M6 7l1 13h10l1-13'],
  folder: ['M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z'],
  refresh: ['M20 12a8 8 0 1 1-2.3-5.6', 'M20 4v4h-4'],
  close: ['M6 6l12 12', 'M18 6 6 18'],
  check: ['M5 13l4 4L19 7'],
  more: ['M6 12h.01', 'M12 12h.01', 'M18 12h.01'],
  chevron: ['M6 9l6 6 6-6'],
  external: ['M14 4h6v6', 'M20 4l-8 8', 'M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4'],
  music: ['M9 18V6l10-2v12', 'M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z', 'M19 16a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z'],
  cloud: ['M6.5 18a4.5 4.5 0 0 1 0-9 6 6 0 0 1 11.4 1.5A3.75 3.75 0 0 1 17.5 18h-11Z'],
  user: ['M12 4a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z', 'M4.5 20a7.5 7.5 0 0 1 15 0'],
  lock: ['M6 11h12v9H6v-9Z', 'M9 11V8a3 3 0 0 1 6 0v3', 'M12 15v2'],
  qr: ['M4 4h5v5H4V4Z', 'M15 4h5v5h-5V4Z', 'M4 15h5v5H4v-5Z', 'M14 14h2v2h-2v-2Z', 'M18 14h2v2h-2v-2Z', 'M14 18h2v2h-2v-2Z', 'M18 18h2v2h-2v-2Z'],
  monitor: ['M3 5h18v11H3z', 'M9 20h6', 'M12 16v4'],
  phone: ['M8 3h8a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z', 'M11 18h2'],
  info: ['M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Z', 'M12 11v5', 'M12 8h.01'],
  alert: ['M12 4 2.5 20h19L12 4Z', 'M12 10v4', 'M12 17h.01'],
  plus: ['M12 5v14', 'M5 12h14'],
  queue: ['M4 7h11', 'M4 12h11', 'M4 17h7', 'M17 13v6', 'M14 16l3-3 3 3'],
  disc: ['M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Z', 'M12 10.5a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3Z'],
  lyric: ['M5 6h9', 'M5 11h14', 'M5 16h7', 'M17 13v6'],
  filter: ['M4 6h16', 'M7 12h10', 'M10 18h4'],
}
</script>

<template>
  <svg
    :width="props.size"
    :height="props.size"
    viewBox="0 0 24 24"
    :fill="props.filled ? 'currentColor' : 'none'"
    stroke="currentColor"
    :stroke-width="props.filled ? 0 : 1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path v-for="(d, i) in PATHS[props.name] ?? []" :key="i" :d="d" />
  </svg>
</template>
