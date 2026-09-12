<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useProfilesStore } from "../stores/profiles";
import { setProfileSecret } from "../lib/api";

const store = useProfilesStore();

const profile = computed(() =>
  store.profiles.find((p) => p.id === store.authPromptId) ?? null,
);
const mode = computed<"password" | "key" | "passphrase">(() => {
  const p = profile.value;
  if (!p) return "password";
  const auth = p.auth_mode ?? (p.private_key ? "key" : "password");
  if (auth === "key") {
    return p.private_key ? "passphrase" : "key";
  }
  return "password";
});
const open_ = computed(() => store.authPromptId != null && profile.value != null);

const password = ref("");
const passphrase = ref("");
const privateKey = ref("");
const remember = ref(false);
const submitting = ref(false);

watch(open_, (v) => {
  if (v) {
    password.value = "";
    passphrase.value = "";
    privateKey.value = "";
    remember.value = false;
  }
});

async function pickKey() {
  // 延迟打开原生对话框，避免与点击事件级联竞争输入
  await new Promise((r) => setTimeout(r, 150));
  try {
    const path = await open({ multiple: false });
    if (path && typeof path === "string") {
      privateKey.value = path;
    }
  } catch {
    /* ignore */
  }
}

async function connect() {
  const id = store.authPromptId;
  if (!id || submitting.value) return;
  const m = mode.value;
  const secrets: { password?: string; passphrase?: string; private_key?: string } = {};
  const overrides: typeof secrets = {};
  if (m === "password") {
    if (!password.value) return;
    secrets.password = password.value;
    overrides.password = password.value;
  } else if (m === "key") {
    if (!privateKey.value) return;
    secrets.private_key = privateKey.value;
    overrides.private_key = privateKey.value;
    if (passphrase.value) {
      secrets.passphrase = passphrase.value;
      overrides.passphrase = passphrase.value;
    }
  } else {
    if (!passphrase.value) return;
    secrets.passphrase = passphrase.value;
    overrides.passphrase = passphrase.value;
  }
  submitting.value = true;
  store.authPromptId = null;
  try {
    if (remember.value) {
      await setProfileSecret(id, secrets);
    }
  } catch {
    /* 保存失败不影响连接 */
  }
  void store.doLaunch(id, overrides);
}

function cancel() {
  store.authPromptId = null;
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open_" class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">连接认证</span>
          <button class="dialog-close" @click="cancel">✕</button>
        </div>
        <div class="dialog-body">
          <div class="target-line">
            {{ profile?.username ? `${profile.username}@` : "" }}{{ profile?.host ?? profile?.name }}
          </div>

          <template v-if="mode === 'password'">
            <div class="form-row">
              <label class="form-label">密码</label>
              <input
                v-model="password"
                class="form-input"
                type="password"
                autofocus
                @keyup.enter="connect"
              />
            </div>
            <label class="check-row">
              <input v-model="remember" type="checkbox" />
              <span>记住密码（加密保存到会话配置）</span>
            </label>
          </template>

          <template v-else-if="mode === 'key'">
            <div class="form-row">
              <label class="form-label">私钥</label>
              <input class="form-input" :value="privateKey" type="text" readonly placeholder="未选择私钥文件" />
              <button class="btn" @click="pickKey">选择…</button>
            </div>
            <div class="form-row">
              <label class="form-label">口令</label>
              <input v-model="passphrase" class="form-input" type="password" placeholder="可选" />
            </div>
            <label class="check-row">
              <input v-model="remember" type="checkbox" />
              <span>记住私钥路径与口令</span>
            </label>
          </template>

          <template v-else>
            <div class="form-row">
              <label class="form-label">口令</label>
              <input
                v-model="passphrase"
                class="form-input"
                type="password"
                @keyup.enter="connect"
              />
            </div>
            <label class="check-row">
              <input v-model="remember" type="checkbox" />
              <span>记住口令（加密保存到会话配置）</span>
            </label>
          </template>
        </div>
        <div class="dialog-footer">
          <button class="btn" @click="cancel">取消</button>
          <button class="btn primary" :disabled="submitting" @click="connect">连接</button>
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
  width: 400px;
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
  margin-bottom: 14px;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}

.form-label {
  width: 44px;
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
  box-sizing: border-box;
}

.form-input:focus {
  border-color: #4ec9b0;
}

.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: #aaaaaa;
  cursor: pointer;
  user-select: none;
}

.btn {
  padding: 6px 14px;
  font-size: 12px;
  color: #cccccc;
  background: transparent;
  border: 1px solid #555555;
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
}

.btn:hover {
  background: #3c3c3c;
  color: #ffffff;
}

.btn.primary {
  color: #1e1e1e;
  background: #4ec9b0;
  border-color: #4ec9b0;
  font-weight: 600;
}

.btn.primary:hover {
  background: #5fd8be;
  color: #1e1e1e;
}

.btn:disabled {
  opacity: 0.6;
  cursor: default;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid #333333;
}
</style>
