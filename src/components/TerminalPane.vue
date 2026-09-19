<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from "vue";
import { useSessionStore } from "../stores/session";
import { useProfilesStore } from "../stores/profiles";
import type { SessionInfo } from "../types/session";
import TerminalTab from "./TerminalTab.vue";

const store = useSessionStore();
const profilesStore = useProfilesStore();

const emit = defineEmits<{
  (e: "new-session"): void;
}>();

const tabRefs = ref<Map<string, InstanceType<typeof TerminalTab>>>(new Map());
const stripRef = ref<HTMLElement | null>(null);
const canScrollLeft = ref(false);
const canScrollRight = ref(false);
let scrollObserver: ResizeObserver | null = null;

function baseTitle(s: SessionInfo): string {
  const pid = profilesStore.launchedFrom[s.id];
  const profile = pid ? profilesStore.profiles.find((x) => x.id === pid) : null;
  return profile?.name ?? s.target;
}

// 同名 tab 动态编号：第 1 个不加后缀，后续依次 (2) (3)…
const tabTitles = computed(() => {
  const seen = new Map<string, number>();
  const result = new Map<string, string>();
  for (const s of store.sessions) {
    const base = baseTitle(s);
    const n = (seen.get(base) ?? 0) + 1;
    seen.set(base, n);
    result.set(s.id, n > 1 ? `${base} (${n})` : base);
  }
  return result;
});

function tabTooltip(s: SessionInfo): string {
  const label = tabTitles.value.get(s.id) ?? s.target;
  return label === s.target ? s.target : `${label} · ${s.target}`;
}

function setTabRef(id: string, el: unknown) {
  if (el) {
    tabRefs.value.set(id, el as InstanceType<typeof TerminalTab>);
  } else {
    tabRefs.value.delete(id);
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && (e.key === "n" || e.key === "N")) {
    e.preventDefault();
    emit("new-session");
  }
}

function updateScrollArrows() {
  const strip = stripRef.value;
  if (!strip) return;
  canScrollLeft.value = strip.scrollLeft > 1;
  canScrollRight.value =
    strip.scrollWidth > strip.clientWidth + 1 &&
    strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 1;
}

function scrollStrip(direction: -1 | 1) {
  stripRef.value?.scrollBy({ left: direction * 220, behavior: "smooth" });
}

function scrollToActiveTab() {
  const strip = stripRef.value;
  const active = strip?.querySelector<HTMLElement>(".tab.active");
  if (!strip || !active) return;
  active.scrollIntoView({ inline: "start", block: "nearest", behavior: "smooth" });
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  void store.refresh();
  if (typeof ResizeObserver !== "undefined" && stripRef.value) {
    scrollObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => updateScrollArrows());
    });
    scrollObserver.observe(stripRef.value);
  }
  void nextTick(updateScrollArrows);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  scrollObserver?.disconnect();
  scrollObserver = null;
});

watch(
  () => store.activeId,
  async () => {
    await nextTick();
    scrollToActiveTab();
    updateScrollArrows();
  },
);

watch(
  () => store.sessions.length,
  async () => {
    await nextTick();
    updateScrollArrows();
    if (store.activeId) scrollToActiveTab();
  },
);

function closeTab(id: string) {
  void store.close(id);
  tabRefs.value.delete(id);
}

function stateIcon(state: string): string {
  return state === "connected" ? "✓" : state === "closed" ? "✗" : "◌";
}

</script>

<template>
  <div class="terminal-pane">
    <div class="tab-bar">
      <button
        v-if="store.sessions.length"
        class="tab-arrow"
        title="向左滚动标签页"
        :disabled="!canScrollLeft"
        @click="scrollStrip(-1)"
      >
        ‹
      </button>
      <div ref="stripRef" class="tab-strip" @scroll.passive="updateScrollArrows">
        <div
          v-for="s in store.sessions"
          :key="s.id"
          class="tab"
          :class="{ active: s.id === store.activeId }"
          @click="store.setActive(s.id)"
        >
          <span class="tab-state" :class="s.state">{{ stateIcon(s.state) }}</span>
          <span class="tab-label" :title="tabTooltip(s)">
            {{ tabTitles.get(s.id) ?? s.target }}
          </span>
          <button class="tab-close" @click.stop="closeTab(s.id)">✕</button>
        </div>
      </div>
      <button
        v-if="store.sessions.length"
        class="tab-arrow"
        title="向右滚动标签页"
        :disabled="!canScrollRight"
        @click="scrollStrip(1)"
      >
        ›
      </button>
    </div>
    <div v-if="store.sessions.length === 0" class="empty-state">
      <div class="empty-hint">
        <div class="empty-icon">⌨</div>
        <div>新建会话 (Ctrl+N)</div>
      </div>
    </div>
    <div v-else class="tab-content">
      <template v-for="s in store.sessions" :key="s.id">
        <TerminalTab
          v-show="s.id === store.activeId"
          :ref="(el: unknown) => setTabRef(s.id, el)"
          :session-id="s.id"
          :active="s.id === store.activeId"
        />
      </template>
    </div>
  </div>
</template>

<style scoped>
.terminal-pane {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  background: #1e1e1e;
}

.tab-bar {
  display: flex;
  align-items: stretch;
  height: 28px;
  background: #181818;
  border-bottom: 1px solid #2a2a2a;
  flex-shrink: 0;
}

.tab-arrow {
  width: 36px;
  height: 28px;
  font-size: 14px;
  line-height: 28px;
  text-align: center;
  color: #cccccc;
  background: #181818;
  border: none;
  border-right: 1px solid #2a2a2a;
  cursor: pointer;
  flex-shrink: 0;
}

.tab-arrow:disabled {
  color: #555555;
  cursor: default;
}

.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px 0 12px;
  font-size: 12px;
  color: #999999;
  background: #1e1e1e;
  border-right: 1px solid #2a2a2a;
  cursor: pointer;
  white-space: nowrap;
}

.tab:hover {
  color: #cccccc;
}

.tab.active {
  color: #ffffff;
  background: #1e1e1e;
  border-bottom: 2px solid #4ec9b0;
}

.tab-strip {
  display: flex;
  align-items: stretch;
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.tab-strip::-webkit-scrollbar {
  display: none;
}

.tab-state {
  font-size: 11px;
  flex-shrink: 0;
  line-height: 1;
}

.tab-state.connected {
  color: #4ec9b0;
}

.tab-state.connecting {
  color: #e5c07b;
}

.tab-state.closed {
  color: #ff6b6b;
}

.tab-label {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  font-family: Consolas, monospace;
}

.tab-close {
  width: 16px;
  height: 16px;
  padding: 0;
  font-size: 10px;
  line-height: 16px;
  text-align: center;
  color: #999999;
  background: transparent;
  border: none;
  border-radius: 3px;
  cursor: pointer;
}

.tab-close:hover {
  color: #ffffff;
  background: #444444;
}

.tab-content {
  flex: 1;
  min-height: 0;
  position: relative;
  padding: 5px;
}

.tab-content > :deep(.terminal-tab) {
  position: absolute;
  inset: 0;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
}

.empty-hint {
  text-align: center;
  color: #666666;
  font-size: 14px;
}

.empty-icon {
  font-size: 40px;
  margin-bottom: 12px;
  color: #444444;
}
</style>
