<script setup lang="ts">
import { ref, onMounted } from "vue";
import { getVersion } from "@tauri-apps/api/app";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const version = ref("");
onMounted(async () => {
  try {
    version.value = await getVersion();
  } catch {
    version.value = "";
  }
});

function close() {
  emit("update:open", false);
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="overlay" @click.self="close">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">关于</span>
          <button class="dialog-close" @click="close">✕</button>
        </div>
        <div class="dialog-body">
          <div class="about-row">
            <span class="about-label">版本</span>
            <span class="about-value">{{ version || "—" }}</span>
          </div>
          <div class="about-row">
            <span class="about-label">作者</span>
            <span class="about-value">rayner</span>
          </div>
          <div class="about-row">
            <span class="about-label">邮箱</span>
            <span class="about-value">rayner.luo@qq.com</span>
          </div>
        </div>
        <div class="dialog-footer">
          <button class="btn primary" @click="close">关闭</button>
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

.about-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 10px;
}

.about-label {
  width: 44px;
  flex-shrink: 0;
  font-size: 12px;
  color: #888888;
  text-align: right;
}

.about-value {
  font-size: 12px;
  color: #e0e0e0;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  padding: 12px 16px;
  border-top: 1px solid #333333;
}

.btn.primary {
  padding: 6px 18px;
  font-size: 12px;
  color: #1e1e1e;
  background: #4ec9b0;
  border: 1px solid #4ec9b0;
  border-radius: 4px;
  font-weight: 600;
  cursor: pointer;
}
</style>
