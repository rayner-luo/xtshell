<script setup lang="ts">
import { ref, watch } from "vue";
import { useSessionStore } from "../stores/session";
import { useProfilesStore } from "../stores/profiles";
import { encodingOptions, type Encoding } from "../types/session";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const sessionStore = useSessionStore();
const profilesStore = useProfilesStore();
const selected = ref<Encoding>("utf-8");

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen && sessionStore.activeId) {
      selected.value = sessionStore.encodingOf(sessionStore.activeId);
    }
  },
);

function apply() {
  const id = sessionStore.activeId;
  if (!id) return;
  sessionStore.setEncoding(id, selected.value);
  const sourceProfile = profilesStore.launchedFrom[id];
  if (sourceProfile) {
    void profilesStore.updateProfileEncoding(sourceProfile, selected.value);
  }
  close();
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
          <span class="dialog-title">输出编码</span>
          <button class="dialog-close" @click="close">✕</button>
        </div>
        <div class="dialog-body">
          <div v-if="!sessionStore.activeSession" class="hint-text">当前没有激活会话</div>
          <template v-else>
            <div class="target-line">作用于当前会话：{{ sessionStore.activeSession!.target }}</div>
            <div class="form-row">
              <label class="form-label">编码</label>
              <select v-model="selected" class="form-input narrow">
                <option v-for="e in encodingOptions" :key="e.value" :value="e.value">
                  {{ e.label }}
                </option>
              </select>
            </div>
          </template>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="close">取消</button>
          <button
            class="btn primary"
            :disabled="!sessionStore.activeSession"
            @click="apply"
          >
            确定
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
  width: 320px;
  background: #252525;
  border: 1px solid #3c3c3c;
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid #333333;
}

.dialog-title {
  font-size: 14px;
  font-weight: 600;
  color: #ffffff;
}

.dialog-close {
  width: 22px;
  height: 22px;
  font-size: 12px;
  color: #999999;
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.dialog-close:hover {
  color: #ffffff;
  background: #3c3c3c;
}

.dialog-body {
  padding: 16px;
}

.target-line {
  font-size: 12px;
  color: #4ec9b0;
  margin-bottom: 12px;
  font-family: Consolas, monospace;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.form-label {
  width: 50px;
  flex-shrink: 0;
  font-size: 12px;
  color: #cccccc;
  text-align: right;
}

.form-input {
  flex: 1;
  min-width: 0;
  padding: 6px 10px;
  font-size: 12px;
  color: #e0e0e0;
  background: #1a1a1a;
  border: 1px solid #444444;
  border-radius: 4px;
  outline: none;
}

.form-input:focus {
  border-color: #4ec9b0;
}

.hint-text {
  margin-top: 10px;
  font-size: 11px;
  color: #777777;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid #333333;
}

.btn {
  padding: 6px 18px;
  font-size: 12px;
  color: #cccccc;
  background: #333333;
  border: 1px solid #444444;
  border-radius: 4px;
  cursor: pointer;
}

.btn.primary {
  color: #1e1e1e;
  background: #4ec9b0;
  border-color: #4ec9b0;
  font-weight: 600;
}
</style>
