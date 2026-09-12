<script setup lang="ts">
import { computed } from "vue";
import { useSessionStore } from "../stores/session";
import { useSftpStore } from "../stores/sftp";
import { getTerminalActions } from "../lib/terminalBus";

const store = useSessionStore();
const sftpStore = useSftpStore();

const emit = defineEmits<{
  (e: "new-session"): void;
  (e: "disconnect"): void;
  (e: "reconnect"): void;
  (e: "font-settings"): void;
  (e: "encoding-settings"): void;
  (e: "mcp-info"): void;
}>();

const canReconnect = computed(() => store.activeSession?.state === "closed");
const isSsh = computed(() => store.activeSession?.protocol === "ssh");
const sftpConnected = computed(() => {
  const id = store.activeId;
  return id != null && sftpStore.stateOf(id).connected;
});

/** 工具栏 SFTP 按钮：切到 SFTP 面板并建立连接 */
function toggleSftp() {
  const id = store.activeId;
  if (!id) return;
  void sftpStore.openPanelAndConnect(id);
}

function copySelection() {
  getTerminalActions(store.activeId)?.copy();
}

function pasteToTerminal() {
  getTerminalActions(store.activeId)?.paste();
}
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-left">
      <button class="icon-btn" title="新建" @click="emit('new-session')">
        ➕
      </button>
      <button
        class="icon-btn"
        title="断开"
        :disabled="!store.activeSession"
        @click="emit('disconnect')"
      >
        ⛓️‍💥
      </button>
      <button
        class="icon-btn reconnect"
        title="重连"
        :disabled="!canReconnect"
        @click="emit('reconnect')"
      >
        🔗
      </button>
      <span class="divider"></span>
      <button class="icon-btn" title="字体设置" @click="emit('font-settings')">
        Aa
      </button>
      <button
        class="icon-btn"
        title="输出编码（当前会话）"
        :disabled="!store.activeSession"
        @click="emit('encoding-settings')"
      >
        🌐
      </button>
      <span class="divider"></span>
      <button
        class="icon-btn"
        title="复制选中内容"
        :disabled="!store.activeSession"
        @click="copySelection"
      >
        ⧉
      </button>
      <button
        class="icon-btn"
        title="粘贴"
        :disabled="!store.activeSession"
        @click="pasteToTerminal"
      >
        📋
      </button>
      <span class="divider"></span>
      <button
        class="icon-btn sftp-btn"
        title="连接 SFTP"
        :class="{ active: sftpConnected }"
        :disabled="!isSsh"
        @click="toggleSftp"
      >
        SFTP
      </button>
      <span class="divider"></span>
      <button class="icon-btn mcp-btn" title="MCP 服务信息" @click="emit('mcp-info')">
        MCP
      </button>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 40px;
  padding: 0 8px;
  background: #252525;
  border-bottom: 1px solid #1a1a1a;
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  gap: 6px;
}

.icon-btn {
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 15px;
  color: #cccccc;
  background: #333333;
  border: 1px solid #444444;
  border-radius: 4px;
  cursor: pointer;
}

.icon-btn:hover:not(:disabled) {
  background: #3c3c3c;
  border-color: #555555;
}

.icon-btn:disabled {
  color: #555555;
  background: #2a2a2a;
  cursor: default;
}

.icon-btn.reconnect:hover:not(:disabled) {
  color: #4ec9b0;
  border-color: #2a5a50;
}

.divider {
  width: 1px;
  height: 18px;
  margin: 0 4px;
  background: #444444;
}

.mcp-btn {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.5px;
  color: #9cdcfe;
}

.mcp-btn:hover {
  border-color: #2a5a78;
  color: #c8e6ff;
}

.sftp-btn {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.5px;
  color: #4ec9b0;
}

.sftp-btn.active {
  color: #1e1e1e;
  background: #4ec9b0;
}

.sftp-btn:disabled {
  opacity: 0.4;
}
</style>
