<script setup lang="ts">
import { ref } from "vue";
import { useSettingsStore, fontOptions } from "../stores/settings";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const settings = useSettingsStore();
const previewText = ref("rayner@debian:~$ hostname && uname -a");

function close() {
  emit("update:open", false);
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="overlay" @click.self="close">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">字体设置</span>
          <button class="dialog-close" @click="close">✕</button>
        </div>
        <div class="dialog-body">
          <div class="form-row">
            <label class="form-label">字体族</label>
            <select v-model="settings.fontFamily" class="form-input">
              <option v-for="f in fontOptions" :key="f.value" :value="f.value">
                {{ f.label }}
              </option>
            </select>
          </div>
          <div class="form-row">
            <label class="form-label">字号</label>
            <input
              v-model.number="settings.fontSize"
              class="form-input narrow"
              type="number"
              :min="8"
              :max="32"
            />
          </div>
          <div class="preview-box" :style="{ fontFamily: settings.fontFamily, fontSize: settings.fontSize + 'px' }">
            {{ previewText }}
          </div>
          <div class="hint-text">设置对所有终端（含已打开的）即时生效</div>
        </div>
        <div class="dialog-footer">
          <button class="btn primary" @click="close">完成</button>
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

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.form-label {
  width: 60px;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-secondary);
  text-align: right;
}

.form-input {
  flex: 1;
  min-width: 0;
  padding: 6px 10px;
  font-size: 12px;
  color: var(--text-body);
  background: var(--bg-input);
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  outline: none;
  box-sizing: border-box;
}

.form-input.narrow {
  flex: 0 0 100px;
}

.form-input:focus {
  border-color: var(--accent);
}

.preview-box {
  margin-top: 4px;
  padding: 10px 12px;
  color: var(--accent);
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
  white-space: nowrap;
  overflow: hidden;
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
