<script setup lang="ts">
import { ref } from "vue";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const mcpConfig = JSON.stringify(
  { xtshell: { type: "http", url: "http://127.0.0.1:46543/mcp" } },
  null,
  2
);

const copied = ref(false);
let copiedTimer: ReturnType<typeof setTimeout> | null = null;

async function copyConfig() {
  try {
    await writeText(mcpConfig);
    copied.value = true;
    if (copiedTimer) clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch {
    /* ignore */
  }
}

function close() {
  emit("update:open", false);
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="overlay" @click.self="close">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">MCP 服务</span>
          <button class="dialog-close" @click="close">✕</button>
        </div>
        <div class="dialog-body">
          <pre class="config-block">{{ mcpConfig }}</pre>
          <div class="hint-text">外部 agent 可通过此配置连接本地 MCP 服务</div>
        </div>
        <div class="dialog-footer">
          <button class="btn primary" @click="copyConfig">
            {{ copied ? "已复制✓" : "复制" }}
          </button>
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
  width: 420px;
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
  padding: 16px;
}

.config-block {
  margin: 0;
  padding: 10px 12px;
  font-family: Consolas, "Courier New", monospace;
  font-size: 12px;
  line-height: 1.5;
  color: var(--accent);
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
  white-space: pre;
  overflow-x: auto;
}

.hint-text {
  margin-top: 10px;
  font-size: 11px;
  color: var(--text-faint);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
}

.btn.primary {
  padding: 6px 18px;
  font-size: 12px;
  color: var(--bg-app);
  background: var(--accent);
  border: 1px solid var(--accent);
  border-radius: 4px;
  font-weight: 600;
  cursor: pointer;
}
</style>
