<script setup lang="ts">
import { computed } from "vue";
import { useSessionStore } from "../stores/session";
import { useSftpStore } from "../stores/sftp";
import { getTerminalActions } from "../lib/terminalBus";
import { useSessionLog } from "../composables/useSessionLog";
import { AddIcon, ConnectIcon, CopyIcon, DisconnectIcon, EncodingIcon, FontIcon, LogIcon, McpIcon, PasteIcon, SftpIcon } from "./icons";

const store = useSessionStore();
const sftpStore = useSftpStore();
const { isRecording, logTooltip, toggleLogging } = useSessionLog();

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
        <component :is="AddIcon" />
      </button>
      <button
        class="icon-btn accent"
        title="断开"
        :disabled="!store.activeSession"
        @click="emit('disconnect')"
      >
        <component :is="DisconnectIcon" />
      </button>
      <button
        class="icon-btn accent reconnect"
        title="重连"
        :disabled="!canReconnect"
        @click="emit('reconnect')"
      >
        <component :is="ConnectIcon" />
      </button>
      <span class="divider"></span>
      <button class="icon-btn" title="字体设置" @click="emit('font-settings')">
        <component :is="FontIcon" />
      </button>
      <button
        class="icon-btn accent"
        title="输出编码（当前会话）"
        :disabled="!store.activeSession"
        @click="emit('encoding-settings')"
      >
        <component :is="EncodingIcon" />
      </button>
      <span class="divider"></span>
      <button
        class="icon-btn accent"
        title="复制选中内容"
        :disabled="!store.activeSession"
        @click="copySelection"
      >
        <component :is="CopyIcon" />
      </button>
      <button
        class="icon-btn accent"
        title="粘贴"
        :disabled="!store.activeSession"
        @click="pasteToTerminal"
      >
        <component :is="PasteIcon" />
      </button>
      <span class="divider"></span>
      <button
        class="icon-btn accent sftp-btn"
        title="连接 SFTP"
        :class="{ active: sftpConnected }"
        :disabled="!isSsh"
        @click="toggleSftp"
      >
        <component :is="SftpIcon" />
      </button>
      <span class="divider"></span>
      <button class="icon-btn mcp-btn" title="MCP 服务信息" @click="emit('mcp-info')">
        <component :is="McpIcon" />
      </button>
      <span class="divider"></span>
      <button
        class="icon-btn log-btn"
        :class="{ recording: isRecording }"
        :title="logTooltip"
        :disabled="!store.activeSession"
        @click="toggleLogging"
      >
        <component :is="LogIcon" />
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

.icon-btn.accent:not(:disabled) {
  color: #64dafe;
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
  color: #9cdcfe;
}

.mcp-btn:hover {
  border-color: #2a5a78;
  color: #c8e6ff;
}

.sftp-btn {
  color: #4ec9b0;
}

.sftp-btn.active {
  color: #1e1e1e;
  background: #4ec9b0;
}

.sftp-btn:disabled {
  opacity: 0.4;
}

.log-btn:not(:disabled) {
  color: #64dafe;
}

.log-btn.recording:not(:disabled) {
  color: #ff6b6b;
  border-color: #6a3030;
}
</style>
