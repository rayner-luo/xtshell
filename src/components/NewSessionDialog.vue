<script setup lang="ts">
import { ref, watch, computed, onBeforeUnmount } from "vue";
import { useSessionStore } from "../stores/session";
import { useProfilesStore } from "../stores/profiles";
import { fontOptions, useSettingsStore, type TerminalColorScheme } from "../stores/settings";
import { saveProfile, listSerialPorts } from "../lib/api";
import type { Protocol, SessionConfig } from "../types/session";
import type { PortForward, ProfileSessionView, SerialPortInfo } from "../types/profile";
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
const settingsStore = useSettingsStore();

type TabKey = "general" | "terminal" | "forward";
const activeTab = ref<TabKey>("general");

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
const rememberPassword = ref(false);
const error = ref("");
const submitting = ref(false);
const createFolderId = ref<string | null>(null);

// 终端标签页：编码 / 字体 / 字号 / 配色（不设置则沿用全局）
const sessionEncoding = ref<Encoding>("utf-8");
const sessionFontFamily = ref(settingsStore.fontFamily);
const sessionFontSize = ref(settingsStore.fontSize);
const sessionColorScheme = ref<TerminalColorScheme>(settingsStore.colorScheme);

// 转发标签页：SSH 本地端口转发
const forwards = ref<PortForward[]>([]);

const isEdit = computed(() => !!props.editProfile);
const isSsh = computed(() => protocol.value === "ssh");
const profileHasStoredPassword = computed(
  () => !!(props.editProfile?.has_password || props.editProfile?.has_passphrase),
);
const dialogTitle = computed(() => (isEdit.value ? "编辑会话" : "新建会话"));
const submitLabel = computed(() => (isEdit.value ? "保存" : "连接"));

const tabs = computed(() => [
  { key: "general" as TabKey, label: "常规", disabled: false },
  { key: "terminal" as TabKey, label: "终端", disabled: false },
  { key: "forward" as TabKey, label: "转发", disabled: !isSsh.value },
]);

const colorSchemeOptions: { value: TerminalColorScheme; label: string }[] = [
  { value: "dark", label: "深色" },
  { value: "light", label: "浅色" },
  { value: "solarized-dark", label: "Solarized 深色" },
  { value: "solarized-light", label: "Solarized 浅色" },
];

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

function addForward() {
  forwards.value.push({
    local_addr: "127.0.0.1",
    local_port: 8000 + forwards.value.length,
    remote_addr: "127.0.0.1",
    remote_port: 80,
  });
}

function removeForward(index: number) {
  forwards.value.splice(index, 1);
}

watch(
  () => props.open,
  (isOpen) => {
    if (!isOpen) return;
    error.value = "";
    submitting.value = false;
    activeTab.value = "general";
    const p = props.editProfile;
    const appearance = p?.terminal_appearance;
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
      sessionEncoding.value = (p.encoding as Encoding) ?? (appearance?.encoding as Encoding) ?? "utf-8";
      sessionFontFamily.value = appearance?.font_family ?? settingsStore.fontFamily;
      sessionFontSize.value = appearance?.font_size ?? settingsStore.fontSize;
      sessionColorScheme.value = (appearance?.color_scheme as TerminalColorScheme) ?? settingsStore.colorScheme;
      forwards.value = (p.forwards ?? []).map((f) => ({ ...f }));
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
      // 默认 UTF-8：编码按会话/档案走，不继承上次的临时选择
      sessionEncoding.value = "utf-8";
      sessionFontFamily.value = settingsStore.fontFamily;
      sessionFontSize.value = settingsStore.fontSize;
      sessionColorScheme.value = settingsStore.colorScheme;
      forwards.value = [];
      rememberPassword.value = false;
      createFolderId.value = profilesStore.pendingNewFolderId;
      profilesStore.pendingNewFolderId = null;
    }
    serialPorts.value = [];
    void profilesStore.refresh();
    if (protocol.value === "serial") void refreshPorts();
  },
);

watch(protocol, async (p) => {
  port.value = p === "ssh" ? 22 : p === "telnet" ? 23 : port.value;
  // 非 SSH 没有端口转发：自动退回常规页，避免停在禁用页
  if (p !== "ssh" && activeTab.value === "forward") activeTab.value = "general";
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

    const appearance = {
      font_family: sessionFontFamily.value,
      font_size: sessionFontSize.value,
      encoding: sessionEncoding.value,
      color_scheme: sessionColorScheme.value,
    };
    const forwardList: PortForward[] =
      protocol.value === "ssh" ? forwards.value.filter((f) => f.local_port > 0) : [];

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
        terminal_appearance: appearance,
        forwards: forwardList,
      });
      close();
      return;
    }

    // 新建：先建会话并立即关闭对话框（tab 即时可见），profile 后台保存
    config.forwards = forwardList;
    const sessionId = await store.create(config);
    if (sessionEncoding.value !== "utf-8") {
      store.setEncoding(sessionId, sessionEncoding.value);
    }
    close();
    void saveProfile({
      name: sessionName.value.trim() || targetLabel.value,
      folder: createFolderId.value ?? undefined,
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
      terminal_appearance: appearance,
      forwards: forwardList,
    })
      .then((profileId) => profilesStore.linkSession(sessionId, profileId))
      .catch(() => {
        /* 保存失败静默：连接不受影响 */
      });
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    submitting.value = false;
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") close();
}

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

        <div class="tab-bar">
          <button
            v-for="t in tabs"
            :key="t.key"
            class="tab"
            :class="{ active: activeTab === t.key }"
            :disabled="t.disabled"
            @click="activeTab = t.key"
          >
            {{ t.label }}
          </button>
        </div>

        <div class="dialog-body">
          <!-- 常规：连接信息 -->
          <template v-if="activeTab === 'general'">
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
                <input v-model.number="port" class="form-input narrow" type="number" :min="1" :max="65535" />
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
                    <button class="segment" :class="{ selected: authMode === 'password' }" @click="authMode = 'password'">
                      密码
                    </button>
                    <button class="segment" :class="{ selected: authMode === 'key' }" @click="authMode = 'key'">
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
                    <input v-model="privateKey" class="form-input" placeholder="C:\Users\you\.ssh\id_rsa" />
                  </div>
                  <div class="form-row">
                    <label class="form-label">私钥口令</label>
                    <input v-model="privateKeyPassphrase" class="form-input" type="password" />
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
                  <option v-for="p in parityOptions" :key="p.value" :value="p.value" :disabled="!p.enabled">
                    {{ p.label }}
                  </option>
                </select>
              </div>
            </template>

            <div class="section-divider">保存选项</div>
            <div class="form-row">
              <label class="form-label">会话名称</label>
              <input v-model="sessionName" class="form-input" :placeholder="targetLabel || '自动使用主机名'" />
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
          </template>

          <!-- 终端：编码 / 字体 / 配色 -->
          <template v-else-if="activeTab === 'terminal'">
            <div class="form-row">
              <label class="form-label">编码</label>
              <select v-model="sessionEncoding" class="form-input narrow">
                <option v-for="e in encodingOptions" :key="e.value" :value="e.value">
                  {{ e.label }}
                </option>
              </select>
            </div>
            <div class="form-row">
              <label class="form-label">字体</label>
              <select v-model="sessionFontFamily" class="form-input">
                <option v-for="f in fontOptions" :key="f.value" :value="f.value">
                  {{ f.label }}
                </option>
              </select>
            </div>
            <div class="form-row">
              <label class="form-label">字号</label>
              <input v-model.number="sessionFontSize" class="form-input narrow" type="number" min="8" max="32" />
            </div>
            <div class="form-row">
              <label class="form-label">配色</label>
              <select v-model="sessionColorScheme" class="form-input narrow">
                <option v-for="c in colorSchemeOptions" :key="c.value" :value="c.value">
                  {{ c.label }}
                </option>
              </select>
            </div>
            <div class="hint-line">以上设置仅对本会话生效，未修改时沿用全局设置。</div>
          </template>

          <!-- 转发：SSH 本地端口转发 -->
          <template v-else>
            <div class="forward-head">
              <span class="forward-title">本地端口转发</span>
              <button class="btn small" @click="addForward">+ 添加</button>
            </div>
            <div v-if="!forwards.length" class="forward-empty">
              暂无转发规则。添加后连接时会在本地监听，并经本会话转发到远端。
            </div>
            <div v-else class="forward-table">
              <div class="forward-row head">
                <span>本地地址</span>
                <span>本地端口</span>
                <span>远端地址</span>
                <span>远端端口</span>
                <span></span>
              </div>
              <div v-for="(f, i) in forwards" :key="i" class="forward-row">
                <input v-model="f.local_addr" class="form-input cell" placeholder="127.0.0.1" />
                <input v-model.number="f.local_port" class="form-input cell" type="number" min="1" max="65535" />
                <input v-model="f.remote_addr" class="form-input cell" placeholder="127.0.0.1" />
                <input v-model.number="f.remote_port" class="form-input cell" type="number" min="1" max="65535" />
                <button class="fwd-del" title="删除" @click="removeForward(i)">✕</button>
              </div>
            </div>
            <div class="hint-line">
              例：本地 127.0.0.1:8080 → 远端 127.0.0.1:80，本机访问 127.0.0.1:8080 即等价于在服务器上访问 80 端口。
            </div>
          </template>

          <div v-if="error" class="error-text">
            {{ isEdit ? "保存失败：" : "连接失败：" }}{{ error }}
          </div>
        </div>

        <div class="dialog-footer">
          <button class="btn" @click="close">取消</button>
          <button class="btn primary" :disabled="!canSubmit || submitting" @click="submit">
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
  width: 560px;
  max-height: 82vh;
  display: flex;
  flex-direction: column;
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

.tab-bar {
  display: flex;
  gap: 2px;
  padding: 8px 16px 0;
  border-bottom: 1px solid var(--border-subtle);
}

.tab {
  padding: 6px 18px;
  font-size: 12px;
  color: var(--text-muted);
  background: transparent;
  border: 1px solid transparent;
  border-bottom: none;
  border-radius: 4px 4px 0 0;
  cursor: pointer;
}

.tab:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.tab.active {
  color: var(--accent);
  background: var(--bg-panel);
  border-color: var(--border-subtle);
  font-weight: 600;
}

.tab:disabled {
  color: var(--text-disabled);
  cursor: not-allowed;
}

.dialog-body {
  padding: 16px;
  overflow-y: auto;
}

.segment {
  flex: 1;
  padding: 6px 0;
  font-size: 12px;
  color: var(--text-muted);
  background: var(--border-divider);
  border: none;
  cursor: pointer;
}

.segment + .segment {
  border-left: 1px solid var(--border-strong);
}

.segment.selected {
  background: var(--bg-selected);
  color: var(--accent);
  font-weight: 600;
}

.auth-toggle {
  display: flex;
  flex: 1;
  border: 1px solid var(--border-strong);
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
  color: var(--text-secondary);
  text-align: right;
}

.form-label.required::after {
  content: " *";
  color: var(--danger);
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
  box-sizing: border-box;
}

.form-input.narrow {
  flex: 0 0 140px;
}

.form-input:focus {
  border-color: var(--accent);
}

.btn.refresh {
  width: 30px;
  height: 28px;
  padding: 0;
  font-size: 14px;
  flex-shrink: 0;
}

.btn.small {
  padding: 4px 12px;
  font-size: 11px;
}

.section-divider {
  font-size: 11px;
  color: var(--text-faint);
  margin: 16px 0 10px;
  padding-bottom: 6px;
  border-bottom: 1px dashed var(--border-strong);
}

.remember-check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-muted);
  cursor: pointer;
}

.remember-check input {
  accent-color: var(--accent);
}

.hint-line {
  margin-top: 12px;
  font-size: 11px;
  line-height: 1.6;
  color: var(--text-faint);
}

.forward-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.forward-title {
  font-size: 12px;
  color: var(--text-secondary);
}

.forward-empty {
  padding: 18px 10px;
  font-size: 12px;
  color: var(--text-faint);
  text-align: center;
  border: 1px dashed var(--border-strong);
  border-radius: 4px;
}

.forward-table {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.forward-row {
  display: grid;
  grid-template-columns: 1.2fr 78px 1.2fr 78px 26px;
  gap: 6px;
  align-items: center;
}

.forward-row.head span {
  font-size: 11px;
  color: var(--text-faint);
}

.form-input.cell {
  width: 100%;
  flex: none;
  padding: 5px 8px;
  font-size: 11.5px;
}

.fwd-del {
  width: 26px;
  height: 26px;
  font-size: 11px;
  color: var(--text-muted);
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.fwd-del:hover {
  color: var(--danger-bright);
  background: var(--bg-hover);
}

.error-text {
  margin-top: 12px;
  padding: 8px 10px;
  font-size: 12px;
  color: var(--danger-bright);
  background: var(--bg-danger-tint);
  border: 1px solid var(--border-danger-tint);
  border-radius: 4px;
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

.btn.primary:hover:not(:disabled) {
  background: var(--accent-hover);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
