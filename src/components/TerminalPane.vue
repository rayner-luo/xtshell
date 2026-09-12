<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useSessionStore } from "../stores/session";
import { useProfilesStore } from "../stores/profiles";
import type { SessionInfo } from "../types/session";
import TerminalTab from "./TerminalTab.vue";
import { setSessionLog } from "../lib/api";
import { open as pickDir } from "@tauri-apps/plugin-dialog";

const store = useSessionStore();
const profilesStore = useProfilesStore();

const emit = defineEmits<{
  (e: "new-session"): void;
}>();

const tabRefs = ref<Map<string, InstanceType<typeof TerminalTab>>>(new Map());

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

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  void store.refresh();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});

function closeTab(id: string) {
  void store.close(id);
  tabRefs.value.delete(id);
}

function stateIcon(state: string): string {
  return state === "connected" ? "✓" : state === "closed" ? "✗" : "◌";
}

async function toggleLogging() {
  const id = store.activeId;
  if (!id) return;
  const current = store.logging[id];
  try {
    if (current) {
      await setSessionLog(id, null);
      store.setLogging(id, null);
    } else {
      const dir = await pickDir({
        directory: true,
        title: "选择会话日志保存目录",
      });
      if (!dir) return;
      const path = await setSessionLog(id, dir);
      store.setLogging(id, path ?? null);
    }
  } catch {
    /* 选目录取消或命令失败时静默 */
  }
}
</script>

<template>
  <div class="terminal-pane">
    <div class="tab-bar">
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
      <span class="spacer"></span>
      <button
        v-if="store.activeSession"
        class="log-btn"
        :class="{ recording: store.logging[store.activeId!] }"
        :title="store.logging[store.activeId!] ? `停止记录\n${store.logging[store.activeId!]}` : '记录会话日志'"
        @click="toggleLogging"
      >
        ●
      </button>
    </div>
    <div v-if="store.sessions.length === 0" class="empty-state">
      <div class="empty-hint">
        <div class="empty-icon">⌨</div>
        <div>新建会话开始使用 (Ctrl+N)</div>
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
  overflow-x: auto;
  flex-shrink: 0;
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

.spacer {
  flex: 1;
}

.log-btn {
  width: 22px;
  height: 20px;
  margin-right: 8px;
  font-size: 12px;
  line-height: 1;
  color: #777777;
  background: transparent;
  border: 1px solid #3c3c3c;
  border-radius: 3px;
  cursor: pointer;
  flex-shrink: 0;
  align-self: center;
}

.log-btn:hover {
  color: #ffffff;
  border-color: #555555;
}

.log-btn.recording {
  color: #ff6b6b;
  border-color: #6a3030;
  animation: pulse 1.2s ease-in-out infinite;
}

@keyframes pulse {
  50% {
    opacity: 0.45;
  }
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
