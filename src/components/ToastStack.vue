<script setup lang="ts">
import { computed } from 'vue'

import { useTasksStore } from '@/stores/tasks'

const tasks = useTasksStore()

const visible = computed(() => tasks.notices.slice(-3))
</script>

<template>
  <div class="toasts" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="item in visible" :key="item.id" class="toast" :class="item.level">
        <span class="dot" />
        <span class="text">{{ item.message }}</span>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toasts {
  position: fixed;
  right: 18px;
  bottom: calc(var(--player) + 18px);
  display: flex;
  flex-direction: column;
  gap: 8px;
  /* 要压在更新提示（60）上面，否则刚入队的提示会被盖住 */
  z-index: 70;
  pointer-events: none;
}

.toast {
  display: flex;
  align-items: center;
  gap: 9px;
  max-width: 340px;
  padding: 9px 14px 9px 12px;
  border-radius: var(--r-md);
  background: var(--surface);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-lg);
  font-size: 12.5px;
  color: var(--text);
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-3);
  flex: 0 0 auto;
}
.toast.success .dot {
  background: var(--accent-2);
}
.toast.error .dot {
  background: var(--danger);
}
.toast.warn .dot {
  background: var(--warning);
}

.toast-enter-active,
.toast-leave-active {
  transition: all 240ms var(--ease);
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px) scale(0.98);
}

@media (max-width: 860px) {
  .toasts {
    left: 12px;
    right: 12px;
    /* 手机上还要让开底部那条更新提示，再往上抬一点 */
    bottom: calc(var(--player) + 118px);
  }
  .toast {
    max-width: none;
  }
}
</style>
