<script setup lang="ts">
import { computed, ref } from "vue";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const MCP_NAME = "xtshell";
const MCP_TYPE = "http";
const MCP_URL = "http://127.0.0.1:9567/mcp";

/** 客户端配置（JSON），供「复制配置」写入 MCP 客户端配置文件 */
const mcpConfig = computed(() =>
  JSON.stringify({ [MCP_NAME]: { type: MCP_TYPE, url: MCP_URL } }, null, 2),
);

const copied = ref<"" | "url" | "config">("");
let copiedTimer: ReturnType<typeof setTimeout> | null = null;

async function copy(text: string, kind: "url" | "config") {
  try {
    await writeText(text);
    copied.value = kind;
    if (copiedTimer) clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => {
      copied.value = "";
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
          <div class="field-row">
            <span class="field-label">名称</span>
            <span class="field-value">{{ MCP_NAME }}</span>
          </div>
          <div class="field-row">
            <span class="field-label">类型</span>
            <span class="field-value">{{ MCP_TYPE }}</span>
          </div>
          <div class="field-row">
            <span class="field-label">URL</span>
            <span class="field-value url">{{ MCP_URL }}</span>
          </div>
          <div class="hint-text">外部 agent 可通过此地址连接本地 MCP 服务</div>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="copy(MCP_URL, 'url')">
            {{ copied === "url" ? "已复制✓" : "复制 URL" }}
          </button>
          <button class="btn primary" @click="copy(mcpConfig, 'config')">
            {{ copied === "config" ? "已复制✓" : "复制配置" }}
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
  width: 460px;
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
  padding: 6px 16px 12px;
}

.field-row {
  display: flex;
  align-items: baseline;
  gap: 12px;
  padding: 9px 0;
  border-bottom: 1px dashed var(--border-subtle);
}

.field-label {
  flex: 0 0 52px;
  font-size: 12px;
  color: var(--text-secondary);
}

.field-value {
  flex: 1;
  font-size: 12px;
  color: var(--text-primary);
  user-select: text;
}

.field-value.url {
  font-family: Consolas, "Courier New", monospace;
  color: var(--accent);
  word-break: break-all;
}

.hint-text {
  margin-top: 10px;
  font-size: 11px;
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
