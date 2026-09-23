<script setup lang="ts">
import { ref, watch, nextTick } from "vue";
import { useXterm } from "../composables/useXterm";
import { useSessionStore } from "../stores/session";

const props = defineProps<{
  sessionId: string;
  active: boolean;
}>();

const store = useSessionStore();

const container = ref<HTMLElement | null>(null);

const { fit, backgroundColor } = useXterm(
  props.sessionId,
  container,
  () => {
    void store.refresh();
  },
  () => {},
);

watch(
  () => props.active,
  (isActive) => {
    if (isActive) {
      nextTick(() => fit());
    }
  },
  { immediate: false },
);

void store.refresh();

defineExpose({ fit });
</script>

<template>
  <div ref="container" class="terminal-tab" :style="{ backgroundColor }"></div>
</template>

<style scoped>
.terminal-tab {
  width: 100%;
  height: 100%;
  padding: 5px;
  box-sizing: border-box;
  background-color: var(--bg-app);
}
</style>
