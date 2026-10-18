<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { onSshHostKeyPrompt, sshHostKeyDecision, type SshHostKeyPrompt } from "../lib/api";

// 多个 SSH 会话可能同时等待确认：按到达顺序排队，逐个裁决
const queue = ref<SshHostKeyPrompt[]>([]);
const current = ref<SshHostKeyPrompt | null>(null);
let unlisten: (() => void) | null = null;

function next() {
  current.value = queue.value.shift() ?? null;
}

async function decide(accept: boolean) {
  const p = current.value;
  current.value = null;
  if (p) {
    await sshHostKeyDecision(p.session_id, accept).catch(() => {});
  }
  next();
}

onMounted(async () => {
  unlisten = await onSshHostKeyPrompt((p) => {
    if (current.value) queue.value.push(p);
    else current.value = p;
  });
});

onBeforeUnmount(() => {
  unlisten?.();
  unlisten = null;
});
</script>

<template>
  <Teleport to="body">
    <div v-if="current" class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">确认主机身份</span>
        </div>
        <div class="dialog-body">
          <div class="target-line">{{ current.host }}:{{ current.port }}</div>
          <div class="hint-text">
            这是首次连接该主机，请核对服务器指纹是否与预期一致。接受后会记录到
            known_hosts，后续连接将自动校验。
          </div>
          <div class="fp-block">
            <div class="fp-row">
              <span class="fp-label">算法</span>
              <span class="fp-value">{{ current.key_type }}</span>
            </div>
            <div class="fp-row">
              <span class="fp-label">指纹</span>
              <span class="fp-value">{{ current.fingerprint }}</span>
            </div>
          </div>
          <div class="hint-text">{{ current.known_hosts }}</div>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="decide(false)">取消</button>
          <button class="btn primary" @click="decide(true)">接受并连接</button>
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
  width: 440px;
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

.target-line {
  font-size: 12px;
  color: var(--accent);
  margin-bottom: 12px;
  font-family: Consolas, monospace;
}

.fp-block {
  margin: 12px 0;
  padding: 8px 10px;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
}

.fp-row {
  display: flex;
  gap: 10px;
  font-size: 12px;
  line-height: 1.8;
}

.fp-label {
  width: 40px;
  flex-shrink: 0;
  color: var(--text-faint);
}

.fp-value {
  color: var(--text-body);
  font-family: Consolas, monospace;
  word-break: break-all;
}

.hint-text {
  font-size: 11px;
  line-height: 1.6;
  color: var(--text-muted);
  word-break: break-all;
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
