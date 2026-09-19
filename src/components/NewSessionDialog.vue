<script setup lang="ts">
import { ref, watch, computed, onBeforeUnmount } from "vue";
import { useSessionStore } from "../stores/session";
import { useProfilesStore } from "../stores/profiles";
import { saveProfile, listSerialPorts } from "../lib/api";
import type { Protocol, SessionConfig } from "../types/session";
import type { ProfileSessionView, SerialPortInfo } from "../types/profile";
import { encodingOptions, type Encoding } from "../types/session";

const props = defineProps<{
  open: boolean;
  editProfile?: ProfileSessionView | null;
}>();

const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const store = useSessionStore();
const profilesStore = useProfilesStore();

const protocol = ref<Protocol>("ssh");
const host = ref("");
const port = ref(22);
const username = ref("");
const authMode = ref<"password" | "key">("password");
const password = ref("");
const privateKey = ref("");
const privateKeyPassphrase = ref("");
const serialPort = ref("");
const serialPorts = ref<SerialPortInfo[]>([]);
const portsLoading = ref(false);
const baudRate = ref(9600);
const dataBits = ref(8);
const stopBits = ref(1);
const parity = ref("None");
const sessionName = ref("");
const sessionEncoding = ref<Encoding>("utf-8");
const rememberPassword = ref(false);
const error = ref("");
const submitting = ref(false);

const isEdit = computed(() => !!props.editProfile);
const profileHasStoredPassword = computed(
  () => !!(props.editProfile?.has_password || props.editProfile?.has_passphrase),
);
const dialogTitle = computed(() => (isEdit.value ? "编辑会话" : "新建会话"));
const submitLabel = computed(() => (isEdit.value ? "保存" : "连接"));

const baudOptions = [115200, 57600, 38400, 19200, 9600, 4800];
const dataBitsOptions = [8, 7, 6, 5];
const stopBitsOptions = [1, 2];
const parityOptions = [
  { value: "None", label: "None", enabled: true },
  { value: "Odd", label: "Odd", enabled: true },
  { value: "Even", label: "Even", enabled: true },
  { value: "Mark", label: "Mark（库暂不支持）", enabled: false },
  { value: "Space", label: "Space（库暂不支持）", enabled: false },
];

const targetLabel = computed(() => {
  if (protocol.value === "serial") return serialPort.value.trim();
  return host.value.trim() || "";
});

const canSubmit = computed(() => {
  if (protocol.value === "ssh") {
    return host.value.trim() !== "" && username.value.trim() !== "";
  }
  if (protocol.value === "telnet") {
    return host.value.trim() !== "";
  }
  return serialPort.value.trim() !== "";
});

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      error.value = "";
      submitting.value = false;
      const p = props.editProfile;
      if (p) {
        protocol.value = p.protocol;
        host.value = p.host ?? "";
        port.value = p.port ?? 22;
        username.value = p.username ?? "";
        authMode.value = p.private_key ? "key" : "password";
        password.value = "";
        privateKey.value = p.private_key ?? "";
        privateKeyPassphrase.value = "";
        serialPort.value = p.serial_port ?? "";
        baudRate.value = p.baud_rate ?? 9600;
        dataBits.value = p.data_bits ?? 8;
        stopBits.value = p.stop_bits ?? 1;
        parity.value = p.parity ?? "None";
        sessionName.value = p.name;
        sessionEncoding.value = (p.encoding as Encoding) ?? "utf-8";
        rememberPassword.value = p.has_password || p.has_passphrase;
      } else {
        protocol.value = "ssh";
        host.value = "";
        port.value = 22;
        username.value = "";
        authMode.value = "password";
        password.value = "";
        privateKey.value = "";
        privateKeyPassphrase.value = "";
        serialPort.value = "";
        baudRate.value = 9600;
        dataBits.value = 8;
        stopBits.value = 1;
        parity.value = "None";
        sessionName.value = "";
        sessionEncoding.value = "utf-8";
        rememberPassword.value = false;
      }
      serialPorts.value = [];
      void profilesStore.refresh();
      if (protocol.value === "serial") void refreshPorts();
    }
  },
);

watch(protocol, async (p) => {
  port.value = p === "ssh" ? 22 : p === "telnet" ? 23 : port.value;
  if (p === "serial") {
    await refreshPorts();
  }
});

async function refreshPorts() {
  portsLoading.value = true;
  try {
    serialPorts.value = await listSerialPorts();
    if (serialPorts.value.length > 0 && !serialPorts.value.some((p) => p.path === serialPort.value)) {
      serialPort.value = serialPorts.value[0].path;
    }
  } catch {
    serialPorts.value = [];
  } finally {
    portsLoading.value = false;
  }
}

function close() {
  emit("update:open", false);
}

async function submit() {
  if (!canSubmit.value || submitting.value) return;
  submitting.value = true;
  error.value = "";
  try {
    const config: SessionConfig = { protocol: protocol.value };
    if (protocol.value === "ssh") {
      config.host = host.value.trim();
      config.port = port.value;
      config.username = username.value.trim();
      if (authMode.value === "password") {
        config.password = password.value;
      } else {
        config.private_key = privateKey.value.trim();
        config.private_key_passphrase = privateKeyPassphrase.value;
      }
    } else if (protocol.value === "telnet") {
      config.host = host.value.trim();
      config.port = port.value;
      const telnetUser = username.value.trim();
      config.username = telnetUser ? telnetUser : undefined;
      config.password = password.value ? password.value : undefined;
    } else {
      config.serial_port = serialPort.value.trim();
      config.baud_rate = baudRate.value;
      config.data_bits = dataBits.value;
      config.stop_bits = stopBits.value;
      config.parity = parity.value;
    }
    if (props.editProfile) {
      await saveProfile({
        id: props.editProfile.id,
        name: sessionName.value.trim() || props.editProfile.name,
        folder: props.editProfile.folder ?? undefined,
        protocol: config.protocol,
        auth_mode: config.protocol === "ssh" ? (config.private_key ? "key" : "password") : undefined,
        host: config.host,
        port: config.port,
        username: config.username,
        password: config.password,
        remember_password: rememberPassword.value,
        private_key: config.private_key,
        private_key_passphrase: config.private_key_passphrase,
        serial_port: config.serial_port,
        baud_rate: config.baud_rate,
        data_bits: config.data_bits,
        stop_bits: config.stop_bits,
        parity: config.parity,
        encoding: sessionEncoding.value === "utf-8" ? undefined : sessionEncoding.value,
      });
    } else {
      // 先建会话并立即关闭对话框（tab 即时可见），profile 后台保存
      const sessionId = await store.create(config);
      if (sessionEncoding.value !== "utf-8") {
        store.setEncoding(sessionId, sessionEncoding.value);
      }
      close();
      void saveProfile({
        name: sessionName.value.trim() || targetLabel.value,
        protocol: config.protocol,
        auth_mode: config.protocol === "ssh" ? (config.private_key ? "key" : "password") : undefined,
        host: config.host,
        port: config.port,
        username: config.username,
        password: config.password,
        remember_password: rememberPassword.value,
        private_key: config.private_key,
        private_key_passphrase: config.private_key_passphrase,
        serial_port: config.serial_port,
        baud_rate: config.baud_rate,
        data_bits: config.data_bits,
        stop_bits: config.stop_bits,
        parity: config.parity,
        encoding: sessionEncoding.value === "utf-8" ? undefined : sessionEncoding.value,
      })
        .then((profileId) => profilesStore.linkSession(sessionId, profileId))
        .catch(() => {
          /* 保存失败静默：连接不受影响 */
        });
      return;
    }
    close();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    submitting.value = false;
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") close();
}

// 全局监听 Esc：焦点不在对话框内时也能关闭
watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      window.addEventListener("keydown", onKeydown);
    } else {
      window.removeEventListener("keydown", onKeydown);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <span class="dialog-title">{{ dialogTitle }}</span>
          <button class="dialog-close" @click="close">✕</button>
        </div>

        <div class="dialog-body">
          <div class="form-row">
            <label class="form-label required">协议</label>
            <select v-model="protocol" class="form-input narrow">
              <option value="ssh">SSH</option>
              <option value="telnet">Telnet</option>
              <option value="serial">Serial</option>
            </select>
          </div>

          <template v-if="protocol === 'ssh' || protocol === 'telnet'">
            <div class="form-row">
              <label class="form-label required">主机</label>
              <input v-model="host" class="form-input" placeholder="example.com" />
            </div>
            <div class="form-row">
              <label class="form-label">端口</label>
              <input
                v-model.number="port"
                class="form-input narrow"
                type="number"
                :min="1"
                :max="65535"
              />
            </div>
            <div v-if="protocol === 'telnet'" class="form-row">
              <label class="form-label">账号</label>
              <input v-model="username" class="form-input" placeholder="root" />
            </div>
            <div v-if="protocol === 'telnet'" class="form-row">
              <label class="form-label">密码</label>
              <input v-model="password" class="form-input" type="password" />
            </div>

            <div v-if="protocol === 'ssh'" class="form-row">
              <label class="form-label required">用户名</label>
              <input v-model="username" class="form-input" placeholder="root" />
            </div>

            <template v-if="protocol === 'ssh'">
              <div class="form-row">
                <label class="form-label">认证方式</label>
                <div class="auth-toggle">
                  <button
                    class="segment"
                    :class="{ selected: authMode === 'password' }"
                    @click="authMode = 'password'"
                  >
                    密码
                  </button>
                  <button
                    class="segment"
                    :class="{ selected: authMode === 'key' }"
                    @click="authMode = 'key'"
                  >
                    私钥
                  </button>
                </div>
              </div>
              <div v-if="authMode === 'password'" class="form-row">
                <label class="form-label">密码</label>
                <input v-model="password" class="form-input" type="password" />
              </div>
              <template v-else>
                <div class="form-row">
                  <label class="form-label required">私钥路径</label>
                  <input
                    v-model="privateKey"
                    class="form-input"
                    placeholder="C:\Users\you\.ssh\id_rsa"
                  />
                </div>
                <div class="form-row">
                  <label class="form-label">私钥口令</label>
                  <input
                    v-model="privateKeyPassphrase"
                    class="form-input"
                    type="password"
                  />
                </div>
              </template>
            </template>
          </template>

          <template v-else>
            <div class="form-row">
              <label class="form-label required">串口</label>
              <select v-model="serialPort" class="form-input narrow" :disabled="portsLoading">
                <option v-if="serialPorts.length === 0" value="">
                  {{ portsLoading ? "查询中…" : "未检测到串口" }}
                </option>
                <option v-for="p in serialPorts" :key="p.path" :value="p.path">
                  {{ p.path }}（{{ p.kind }}）
                </option>
              </select>
              <button class="btn refresh" title="重新查询串口" @click="refreshPorts">⟳</button>
            </div>
            <div class="form-row">
              <label class="form-label">波特率</label>
              <select v-model.number="baudRate" class="form-input narrow">
                <option v-for="b in baudOptions" :key="b" :value="b">{{ b }}</option>
              </select>
            </div>
            <div class="form-row">
              <label class="form-label">数据位</label>
              <select v-model.number="dataBits" class="form-input narrow">
                <option v-for="b in dataBitsOptions" :key="b" :value="b">{{ b }}</option>
              </select>
            </div>
            <div class="form-row">
              <label class="form-label">停止位</label>
              <select v-model.number="stopBits" class="form-input narrow">
                <option v-for="b in stopBitsOptions" :key="b" :value="b">{{ b }}</option>
              </select>
            </div>
            <div class="form-row">
              <label class="form-label">校验位</label>
              <select v-model="parity" class="form-input narrow">
                <option
                  v-for="p in parityOptions"
                  :key="p.value"
                  :value="p.value"
                  :disabled="!p.enabled"
                >
                  {{ p.label }}
                </option>
              </select>
            </div>
          </template>

          <div class="save-section">
            <div class="section-divider">保存选项</div>
            <div class="form-row">
              <label class="form-label">会话名称</label>
              <input
                v-model="sessionName"
                class="form-input"
                :placeholder="targetLabel || '自动使用主机名'"
              />
            </div>
            <div class="form-row">
              <label class="form-label">编码</label>
              <select v-model="sessionEncoding" class="form-input narrow">
                <option v-for="e in encodingOptions" :key="e.value" :value="e.value">
                  {{ e.label }}
                </option>
              </select>
            </div>
            <div class="form-row">
              <label class="form-label">记住密码</label>
              <label class="remember-check">
                <input v-model="rememberPassword" type="checkbox" />
                <span>{{
                  isEdit && profileHasStoredPassword
                    ? "已记住密码（留空密码框则保留原密码）"
                    : "加密保存密码/私钥口令（默认不记住）"
                }}</span>
              </label>
            </div>
          </div>

          <div v-if="error" class="error-text">{{ isEdit ? "保存失败：" : "连接失败：" }}{{ error }}</div>
        </div>

        <div class="dialog-footer">
          <button class="btn" @click="close">取消</button>
          <button
            class="btn primary"
            :disabled="!canSubmit || submitting"
            @click="submit"
          >
            {{ submitting ? (isEdit ? "保存中…" : "连接中…") : submitLabel }}
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
  width: 420px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
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
  overflow-y: auto;
}

.segment {
  flex: 1;
  padding: 6px 0;
  font-size: 12px;
  color: #999999;
  background: #2a2a2a;
  border: none;
  cursor: pointer;
}

.segment + .segment {
  border-left: 1px solid #444444;
}

.segment.selected {
  background: #333e4c;
  color: #4ec9b0;
  font-weight: 600;
}

.auth-toggle {
  display: flex;
  flex: 1;
  border: 1px solid #444444;
  border-radius: 6px;
  overflow: hidden;
}

.auth-toggle .segment {
  flex: 1;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.form-label {
  width: 70px;
  flex-shrink: 0;
  font-size: 12px;
  color: #cccccc;
  text-align: right;
}

.form-label.required::after {
  content: " *";
  color: #ff6b6b;
}

.form-input {
  flex: 1;
  min-width: 0;
  padding: 6px 10px;
  font-size: 12px;
  font-family: Consolas, monospace;
  color: #e0e0e0;
  background: #1a1a1a;
  border: 1px solid #444444;
  border-radius: 4px;
  outline: none;
  box-sizing: border-box;
}

.form-input.narrow {
  flex: 0 0 120px;
}

.form-input:focus {
  border-color: #4ec9b0;
}

.btn.refresh {
  width: 30px;
  height: 28px;
  padding: 0;
  font-size: 14px;
  flex-shrink: 0;
}

.error-text {
  margin-top: 8px;
  padding: 8px 10px;
  font-size: 12px;
  color: #ff8a8a;
  background: #3a1f1f;
  border: 1px solid #5a2a2a;
  border-radius: 4px;
}

.save-section {
  margin-top: 4px;
  padding-top: 4px;
}

.section-divider {
  font-size: 11px;
  color: #777777;
  margin: 8px 0 10px;
  padding-bottom: 6px;
  border-bottom: 1px dashed #3c3c3c;
}

.remember-check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: #aaaaaa;
  cursor: pointer;
}

.remember-check input {
  accent-color: #4ec9b0;
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

.btn:hover:not(:disabled) {
  background: #3c3c3c;
}

.btn.primary {
  color: #1e1e1e;
  background: #4ec9b0;
  border-color: #4ec9b0;
  font-weight: 600;
}

.btn.primary:hover:not(:disabled) {
  background: #5fdcc0;
}

.btn:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
