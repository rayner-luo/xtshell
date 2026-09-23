<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useSessionStore } from "../stores/session";
import { useProfilesStore } from "../stores/profiles";
import { useSettingsStore, fontOptions, type TerminalColorScheme } from "../stores/settings";
import { encodingOptions, type Encoding } from "../types/session";
import type { TerminalAppearance } from "../types/profile";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "update:open", value: boolean): void }>();

const sessionStore = useSessionStore();
const profilesStore = useProfilesStore();
const settingsStore = useSettingsStore();

const fontFamily = ref(settingsStore.fontFamily);
const fontSize = ref(settingsStore.fontSize);
const encoding = ref<string>(sessionStore.encodingOf(sessionStore.activeId ?? ""));
const colorScheme = ref<string>(settingsStore.colorScheme);

const activeProfileId = computed(() => {
  const sessionId = sessionStore.activeId;
  if (!sessionId) return null;
  return profilesStore.launchedFrom[sessionId] ?? null;
});

const activeProfile = computed(() => {
  if (!activeProfileId.value) return null;
  return profilesStore.profiles.find((p) => p.id === activeProfileId.value) ?? null;
});

const currentAppearance = computed(() => activeProfile.value?.terminal_appearance ?? null);

function setCurrentValues(current: TerminalAppearance | null, global: {
  fontFamily: string;
  fontSize: number;
  encoding: string;
  colorScheme: string;
}) {
  fontFamily.value = current?.font_family ?? global.fontFamily;
  fontSize.value = current?.font_size ?? global.fontSize;
  encoding.value = current?.encoding ?? global.encoding;
  colorScheme.value = current?.color_scheme ?? global.colorScheme;
}

watch(
  () => props.open,
  (isOpen) => {
    if (!isOpen) return;
    setCurrentValues(currentAppearance.value, {
      fontFamily: settingsStore.fontFamily,
      fontSize: settingsStore.fontSize,
      encoding: sessionStore.encodingOf(sessionStore.activeId ?? ""),
      colorScheme: settingsStore.colorScheme,
    });
  },
);

async function save() {
  settingsStore.fontFamily = fontFamily.value;
  settingsStore.fontSize = fontSize.value;
  if (encoding.value) {
    settingsStore.encoding = encoding.value as Encoding;
  }
  settingsStore.colorScheme = colorScheme.value as TerminalColorScheme;
  if (sessionStore.activeId) {
    sessionStore.setEncoding(sessionStore.activeId, encoding.value as Encoding);
  }
  const profileId = activeProfileId.value;
  if (!profileId) return;
  await profilesStore.setAppearance(profileId, {
    font_family: fontFamily.value,
    font_size: fontSize.value,
    encoding: encoding.value,
    color_scheme: colorScheme.value,
  });
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
          <span class="dialog-title">默认设置</span>
          <button class="dialog-close" @click="close">✕</button>
        </div>
        <div class="dialog-body">
          <div class="form-row">
            <label class="form-label">字体</label>
            <select v-model="fontFamily" class="form-input">
              <option v-for="f in fontOptions" :key="f.value" :value="f.value">
                {{ f.label }}
              </option>
            </select>
          </div>
          <div class="form-row">
            <label class="form-label">字号</label>
            <input v-model.number="fontSize" class="form-input narrow" type="number" :min="8" :max="32" />
          </div>
          <div class="form-row">
            <label class="form-label">编码</label>
            <select v-model="encoding" class="form-input narrow">
              <option v-for="e in encodingOptions" :key="e.value" :value="e.value">
                {{ e.label }}
              </option>
            </select>
          </div>
          <div class="form-row">
            <label class="form-label">外观</label>
            <select v-model="colorScheme" class="form-input">
              <option value="light">Light</option>
              <option value="dark">Dark</option>
              <option value="solarized-dark">Solarized Dark</option>
              <option value="solarized-light">Solarized Light</option>
            </select>
          </div>
        </div>
        <div class="dialog-footer">
          <button class="btn cancel" @click="close">取消</button>
          <button class="btn primary" @click="save">保存</button>
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
  width: 80px;
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
  font-family: Consolas, monospace;
  color: var(--text-body);
  background: var(--bg-input);
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  outline: none;
}

.form-input.narrow {
  flex: 0 0 120px;
}

.form-input:focus {
  border-color: var(--accent);
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
}

.btn {
  padding: 6px 18px;
  font-size: 12px;
  color: var(--text-secondary);
  background: var(--border-subtle);
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  cursor: pointer;
}

.btn:hover:not(:disabled) {
  background: var(--bg-hover);
}

.btn.primary {
  color: var(--bg-app);
  background: var(--accent);
  border-color: var(--accent);
  font-weight: 600;
}

.btn.primary:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
