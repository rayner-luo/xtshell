<script setup lang="ts">
import { computed } from "vue";
import { useZmodemStore } from "../stores/zmodem";
import { zmodemResolveConflict } from "../lib/api";

const store = useZmodemStore();

const current = computed(() => store.conflicts[0] ?? null);

async function chooseOverwrite() {
  const c = store.shiftConflict();
  if (c) void zmodemResolveConflict(c.temp, c.dest, "overwrite").catch(() => {});
}

async function chooseRename() {
  const c = store.shiftConflict();
  if (c) void zmodemResolveConflict(c.temp, c.dest, "rename").catch(() => {});
}

async function chooseSkip() {
  const c = store.shiftConflict();
  if (c) void zmodemResolveConflict(c.temp, c.dest, "skip").catch(() => {});
}
</script>

<template>
  <Teleport to="body">
    <div v-if="current" class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">文件已存在</span>
        </div>
        <div class="dialog-body">
          <div class="conflict-name">{{ current.name }}</div>
          <div class="hint-text">目标位置已存在同名文件：{{ current.dest }}</div>
          <div class="hint-text">（{{ current.size }} 字节已接收）</div>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="chooseSkip">跳过</button>
          <button class="btn" @click="chooseOverwrite">覆盖</button>
          <button class="btn primary" @click="chooseRename">自动改名</button>
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
  width: 380px;
  background: #252525;
  border: 1px solid #3c3c3c;
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
}

.dialog-header {
  padding: 12px 16px;
  border-bottom: 1px solid #333333;
}

.dialog-title {
  font-size: 14px;
  font-weight: 600;
  color: #ffffff;
}

.dialog-body {
  padding: 16px;
}

.conflict-name {
  font-size: 13px;
  color: #e0e0e0;
  font-weight: 600;
  margin-bottom: 6px;
}

.hint-text {
  font-size: 11px;
  color: #999999;
  margin-top: 4px;
  word-break: break-all;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid #333333;
}

.btn {
  padding: 6px 14px;
  font-size: 12px;
  color: #cccccc;
  background: transparent;
  border: 1px solid #555555;
  border-radius: 4px;
  cursor: pointer;
}

.btn:hover {
  background: #3c3c3c;
  color: #ffffff;
}

.btn:nth-of-type(3) {
  color: #1e1e1e;
  background: #4ec9b0;
  border-color: #4ec9b0;
  font-weight: 600;
}

.btn:nth-of-type(3):hover {
  background: #5fd8be;
  color: #1e1e1e;
}
</style>
