<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import Icon from './Icon.vue'
import { transport } from '@/lib/transport'
import { SOURCE_META } from '@/lib/types'
import type { PlatformQuality, QualityLevel, Track, Variant } from '@/lib/types'

const props = defineProps<{
  track: Track
  /** 用户点某个音源芯片时传入，弹窗里预选该平台 */
  initialSource?: string | null
}>()
const emit = defineEmits<{
  (e: 'confirm', payload: { variant: Variant; quality: QualityLevel }): void
  (e: 'close'): void
}>()

/** 各平台的档位表，后端唯一维护 */
const qualities = ref<PlatformQuality[]>([])

/** 当前选中的平台（用 source id） */
const selectedSource = ref<string | null>(null)
/** 各平台选中的音质等级 */
const chosen: Record<string, QualityLevel> = {}

onMounted(async () => {
  qualities.value = await transport.platformQualities().catch(() => [])
  // 默认选中用户点的那个平台，否则选音质最高的那个
  if (props.initialSource && props.track.variants.some((v) => v.source === props.initialSource)) {
    selectedSource.value = props.initialSource
  } else if (props.track.variants.length) {
    selectedSource.value = bestVariant().source
  }
  // 每个平台默认落在它可给到的最高档
  for (const variant of props.track.variants) {
    const table = qualities.value.find((item) => item.source === variant.source)
    if (table?.items.length) chosen[variant.source] = table.items[0].level
  }
})

/** 该平台选中的档位对象 */
function tierOf(source: string) {
  const table = qualities.value.find((item) => item.source === source)
  if (!table?.items.length) return null
  const level = chosen[source]
  return table.items.find((item) => item.level === level) ?? table.items[0]
}

/** 无显式选择时按「免费优先 > 音质高」挑默认平台 */
function bestVariant(): Variant {
  const rankFee = (fee: Variant['fee']) =>
    fee === 'free' ? 0 : fee === 'unknown' ? 1 : fee === 'vip' ? 2 : 3
  return [...props.track.variants].sort(
    (a, b) => rankFee(a.fee) - rankFee(b.fee) || b.bitrate - a.bitrate,
  )[0]
}

const selectedVariant = computed(() =>
  props.track.variants.find((v) => v.source === selectedSource.value),
)

function confirm() {
  const variant = selectedVariant.value
  const tier = selectedSource.value ? tierOf(selectedSource.value) : null
  if (!variant || !tier) return
  emit('confirm', { variant, quality: tier.level })
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') emit('close')
}
onMounted(() => window.addEventListener('keydown', onKeydown))
</script>

<template>
  <div class="backdrop" @click.self="emit('close')">
    <div class="dialog" role="dialog" aria-modal="true">
      <header class="head">
        <div class="head-text">
          <h3>{{ track.title }}</h3>
          <p class="sub">{{ track.artists.join(' / ') || '未知歌手' }}</p>
        </div>
        <button class="icon-btn" type="button" title="关闭" @click="emit('close')">
          <Icon name="close" :size="16" />
        </button>
      </header>

      <div class="body">
        <p class="hint muted">选择下载平台与音质。多平台可取的歌，各平台的音质上限不同。</p>

        <div class="platforms">
          <article
            v-for="variant in track.variants"
            :key="variant.source + variant.id"
            class="platform"
            :class="{ on: selectedSource === variant.source }"
            @click="selectedSource = variant.source"
          >
            <header class="p-head">
              <span class="dot" :style="{ background: SOURCE_META[variant.source].color }" />
              <strong>{{ SOURCE_META[variant.source].label }}</strong>
              <span v-if="variant.fee === 'vip'" class="vip">会员</span>
              <span v-else-if="variant.fee === 'paid'" class="vip">付费</span>
              <span v-if="selectedSource === variant.source" class="tick">
                <Icon name="check" :size="13" />
              </span>
            </header>

            <div class="tiers">
              <button
                v-for="tier in qualities.find((q) => q.source === variant.source)?.items ?? []"
                :key="tier.code"
                class="tier"
                :class="{ on: chosen[variant.source] === tier.level }"
                type="button"
                @click.stop="chosen[variant.source] = tier.level"
              >
                <span class="tier-name">
                  {{ tier.label }}
                  <span v-if="tier.vip" class="vip mini">会员</span>
                </span>
                <span class="tier-detail mono">{{ tier.detail }}</span>
              </button>
            </div>
          </article>
        </div>
      </div>

      <footer class="foot">
        <button class="btn" type="button" @click="emit('close')">取消</button>
        <button class="btn btn-primary" type="button" :disabled="!selectedVariant" @click="confirm">
          <Icon name="download" :size="15" />
          下载
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgba(8, 8, 12, 0.5);
  backdrop-filter: blur(4px);
  animation: fade 160ms var(--ease);
}
.dialog {
  width: min(520px, 100%);
  max-height: min(82dvh, 720px);
  display: flex;
  flex-direction: column;
  border-radius: var(--r-lg);
  border: 1px solid var(--line-strong);
  background: var(--surface);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  animation: pop 180ms var(--ease);
}
@keyframes fade {
  from {
    opacity: 0;
  }
}
@keyframes pop {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.98);
  }
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 18px 12px;
  border-bottom: 1px solid var(--line);
}
.head-text {
  min-width: 0;
}
.head h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.head .sub {
  margin: 3px 0 0;
  font-size: 12px;
  color: var(--text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.icon-btn {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 9px;
  color: var(--text-3);
  flex: 0 0 auto;
}
.icon-btn:hover {
  background: var(--hover);
  color: var(--text);
}

.body {
  padding: 12px 18px;
  overflow-y: auto;
}
.hint {
  margin: 0 0 12px;
  font-size: 12px;
  line-height: 1.6;
}

.platforms {
  display: grid;
  gap: 10px;
}
.platform {
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  padding: 10px 12px 12px;
  cursor: pointer;
  transition: border-color var(--dur) var(--ease), background var(--dur) var(--ease);
}
.platform:hover {
  border-color: color-mix(in srgb, var(--accent) 35%, var(--line));
}
.platform.on {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 6%, transparent);
}
.p-head {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 13px;
  margin-bottom: 9px;
}
.p-head strong {
  font-weight: 600;
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex: 0 0 auto;
}
.tick {
  margin-left: auto;
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--accent);
  color: #fff;
}

.tiers {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.tier {
  display: grid;
  gap: 1px;
  padding: 6px 10px;
  border: 1px solid var(--line);
  border-radius: var(--r-sm);
  background: var(--surface);
  text-align: left;
  cursor: pointer;
  transition: all var(--dur) var(--ease);
}
.tier:hover {
  border-color: color-mix(in srgb, var(--accent) 40%, var(--line));
}
.tier.on {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, var(--surface));
}
.tier-name {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--text);
}
.tier.on .tier-name {
  font-weight: 600;
}
.tier-detail {
  font-size: 10.5px;
  color: var(--text-3);
}

.vip {
  padding: 0 5px;
  border-radius: 4px;
  font-size: 9.5px;
  line-height: 1.6;
  color: #b4802a;
  background: color-mix(in srgb, #e8a33d 18%, transparent);
  font-weight: 400;
}
.vip.mini {
  padding: 0 3px;
  font-size: 9px;
}

.foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 18px 16px;
  border-top: 1px solid var(--line);
}
</style>
