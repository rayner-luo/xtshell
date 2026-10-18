<script setup lang="ts">
import { computed } from "vue";
import { useZmodemStore } from "../stores/zmodem";
import { zmodemCancel } from "../lib/api";
import { getTerminalActions } from "../lib/terminalBus";

const store = useZmodemStore();

const open_ = computed(() => store.pendingId != null);

/// 选定方向后交给 ZmodemPickDialog 取路径（用户手势驱动，此处不弹原生框）
function chooseReceive() {
  const id = store.pendingId;
  store.resolve();
  if (!id) return;
  store.askPick(id, "receive");
}

function chooseSend() {
  const id = store.pendingId;
  store.resolve();
  if (!id) return;
  store.askPick(id, "send");
}

function chooseCancel() {
  const id = store.pendingId;
  store.resolve();
  if (!id) return;
  void zmodemCancel(id).catch(() => {});
  getTerminalActions(id)?.focus();
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open_" class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">检测到 ZMODEM 传输</span>
        </div>
        <div class="dialog-body">
          <div class="hint-text">远端请求 ZMODEM 文件传输，请选择方向：</div>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="chooseCancel">取消</button>
          <button class="btn" @click="chooseSend">发送文件（rz）</button>
          <button class="btn primary" @click="chooseReceive">接收文件（sz）</button>
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
  width: 340px;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
}

.dialog-header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle);
}

.dialog-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.dialog-body {
  padding: 16px;
}

.hint-text {
  font-size: 12px;
  color: var(--text-muted);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
}

.btn {
  padding: 6px 14px;
  font-size: 12px;
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid var(--border-hover);
  border-radius: 4px;
  cursor: pointer;
}

.btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.btn.primary {
  color: var(--bg-app);
  background: var(--accent);
  border-color: var(--accent);
  font-weight: 600;
}

.btn.primary:hover {
  background: var(--accent-hover);
  color: var(--bg-app);
}
</style>
