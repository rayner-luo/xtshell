<script setup lang="ts">
import { computed } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useZmodemStore } from "../stores/zmodem";
import { zmodemStartReceive, zmodemSendFiles, zmodemCancel } from "../lib/api";

const store = useZmodemStore();

const open_ = computed(() => store.pendingId != null);

async function chooseReceive() {
  const id = store.pendingId;
  store.resolve();
  if (!id) return;
  try {
    await new Promise((r) => setTimeout(r, 150));
    const dir = await open({ directory: true });
    if (dir) {
      void zmodemStartReceive(id, dir as string).catch(() => {});
    } else {
      void zmodemCancel(id).catch(() => {});
    }
  } catch {
    void zmodemCancel(id).catch(() => {});
  }
}

async function chooseSend() {
  const id = store.pendingId;
  store.resolve();
  if (!id) return;
  try {
    await new Promise((r) => setTimeout(r, 150));
    const paths = await open({ multiple: true });
    if (paths) {
      const list = Array.isArray(paths) ? paths : [paths];
      void zmodemSendFiles(id, list).catch(() => {});
    } else {
      void zmodemCancel(id).catch(() => {});
    }
  } catch {
    void zmodemCancel(id).catch(() => {});
  }
}

async function chooseCancel() {
  const id = store.pendingId;
  store.resolve();
  if (id) void zmodemCancel(id).catch(() => {});
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
