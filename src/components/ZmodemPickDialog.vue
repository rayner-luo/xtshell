<script setup lang="ts">
import { computed } from "vue";
import { useZmodemStore } from "../stores/zmodem";
import { pickDirectory, pickFiles } from "../lib/nativeDialog";
import { zmodemCancel, zmodemSendFiles, zmodemStartReceive } from "../lib/api";
import { getTerminalActions } from "../lib/terminalBus";

const store = useZmodemStore();

const current = computed(() => store.pick);
const isSend = computed(() => current.value?.intent === "send");
const open_ = computed(() => current.value != null);
const hint = computed(() => (isSend.value ? "请选择文件上传" : "请选择保存位置"));

/// 弹框按钮被点击后会获得焦点，关闭时必须把键盘焦点交还终端，
/// 否则用户敲键盘没有任何反应（看起来像终端卡死）。
function focusTerminal(sessionId: string) {
  getTerminalActions(sessionId)?.focus();
}

/// 继续：弹系统选择框，选好后立即开始传输；在选择框里取消则中止本次传输
async function proceed() {
  const req = current.value;
  if (!req) return;
  store.resolvePick();
  try {
    if (req.intent === "send") {
      const files = await pickFiles();
      if (files && files.length) await zmodemSendFiles(req.sessionId, files);
      else await zmodemCancel(req.sessionId);
    } else {
      const dir = await pickDirectory();
      if (dir) await zmodemStartReceive(req.sessionId, dir);
      else await zmodemCancel(req.sessionId);
    }
  } catch {
    void zmodemCancel(req.sessionId).catch(() => {});
  } finally {
    focusTerminal(req.sessionId);
  }
}

function cancel() {
  const req = current.value;
  store.resolvePick();
  if (!req) return;
  void zmodemCancel(req.sessionId).catch(() => {});
  focusTerminal(req.sessionId);
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open_" class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">
            {{ isSend ? "ZMODEM 上传（远端 rz）" : "ZMODEM 下载（远端 sz）" }}
          </span>
        </div>
        <div class="dialog-body">
          <div class="hint-text">{{ hint }}</div>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="cancel">取消</button>
          <button class="btn primary" @click="proceed">继续</button>
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
  font-size: 13px;
  color: var(--text-body);
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
  white-space: nowrap;
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