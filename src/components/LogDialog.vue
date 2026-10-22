<script setup lang="ts">
import { ref, watch } from "vue";
import { clearAppLog, readAppLog } from "../lib/api";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const path = ref("");
const lines = ref<string[]>([]);
const loading = ref(false);

async function load() {
  loading.value = true;
  try {
    const log = await readAppLog();
    path.value = log.path;
    lines.value = log.lines;
  } catch {
    lines.value = [];
  } finally {
    loading.value = false;
  }
}

async function clear() {
  try {
    await clearAppLog();
  } catch {
    /* ignore */
  }
  await load();
}

function close() {
  emit("update:open", false);
}

watch(
  () => props.open,
  (open) => {
    if (open) void load();
  },
);
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="overlay" @click.self="close">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">启动日志</span>
          <button class="dialog-close" @click="close">✕</button>
        </div>
        <div class="dialog-body">
          <div class="log-path" :title="path">
            {{ path || "（日志文件未初始化）" }}
          </div>
          <div class="log-list">
            <div v-if="!lines.length" class="log-empty">
              {{ loading ? "读取中…" : "暂无日志" }}
            </div>
            <!-- 后端已按「最新在最前」返回，直接顺序渲染即可 -->
            <div v-for="(line, i) in lines" :key="i" class="log-line">{{ line }}</div>
          </div>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="load">刷新</button>
          <button class="btn" @click="clear">清空</button>
          <button class="btn primary" @click="close">关闭</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog {
  display: flex;
  flex-direction: column;
  width: 780px;
  max-width: 92vw;
  max-height: 82vh;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle);
}

.dialog-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.dialog-close {
  width: 22px;
  height: 22px;
  font-size: 12px;
  color: var(--text-muted);
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.dialog-close:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.dialog-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  padding: 14px 16px;
}

.log-path {
  font-family: Consolas, "Courier New", monospace;
  font-size: 11px;
  color: var(--text-faint);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.log-list {
  flex: 1;
  min-height: 260px;
  max-height: 58vh;
  overflow-y: auto;
  padding: 8px 10px;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
}

.log-line {
  font-family: Consolas, "Courier New", monospace;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--text-body);
  white-space: pre-wrap;
  word-break: break-all;
}

.log-empty {
  padding: 4px 0;
  font-size: 12px;
  color: var(--text-faint);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
}

.btn {
  padding: 6px 16px;
  font-size: 12px;
  color: var(--text-primary);
  background: var(--bg-raised);
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  cursor: pointer;
}

.btn:hover {
  background: var(--bg-hover);
}

.btn.primary {
  color: var(--text-on-accent);
  background: var(--accent);
  border-color: var(--accent);
  font-weight: 600;
}

.btn.primary:hover {
  background: var(--accent-hover);
}
</style>
