<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import MenuBar from "./components/MenuBar.vue";
import Toolbar from "./components/Toolbar.vue";
import SessionSidebar from "./components/SessionSidebar.vue";
import TerminalPane from "./components/TerminalPane.vue";
import NewSessionDialog from "./components/NewSessionDialog.vue";
import ContextMenu from "./components/ContextMenu.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import EncodingDialog from "./components/EncodingDialog.vue";
import McpDialog from "./components/McpDialog.vue";
import AboutDialog from "./components/AboutDialog.vue";
import ZmodemDialog from "./components/ZmodemDialog.vue";
import ZmodemConflictDialog from "./components/ZmodemConflictDialog.vue";
import ConnectAuthDialog from "./components/ConnectAuthDialog.vue";
import { useSftpStore } from "./stores/sftp";
import { getTerminalActions } from "./lib/terminalBus";
import { ask } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSessionStore } from "./stores/session";
import { useProfilesStore } from "./stores/profiles";
import { useSettingsStore } from "./stores/settings";
import { useSessionLog } from "./composables/useSessionLog";
import { onSessionsChanged, onProfilesChanged } from "./lib/api";

const store = useSessionStore();
const sftpStore = useSftpStore();
const settingsStore = useSettingsStore();
const sessionLog = useSessionLog();

function onMenuSftp() {
  const id = store.activeId;
  if (id) void sftpStore.openPanelAndConnect(id);
}
const profilesStore = useProfilesStore();
const showDialog = ref(false);
const showEncoding = ref(false);
const showSettings = ref(false);
const showMcp = ref(false);
const showAbout = ref(false);
const dialogOpen = computed({
  get: () => showDialog.value || profilesStore.editingProfile != null,
  set: (v: boolean) => {
    showDialog.value = v;
    if (!v) profilesStore.clearEdit();
  },
});
let unlistenSessions: (() => void) | null = null;
let unlistenProfiles: (() => void) | null = null;

function newSession() {
  showDialog.value = true;
}

function onNewSessionFromMenu() {
  showDialog.value = true;
}

function disconnect() {
  if (store.activeId) {
    void store.close(store.activeId);
  }
}

function reconnect() {
  if (store.activeId) {
    void store.reconnect(store.activeId);
  }
}

function menuCopy() {
  const id = store.activeId;
  if (id) getTerminalActions(id)?.copy();
}

function menuPaste() {
  const id = store.activeId;
  if (id) getTerminalActions(id)?.paste();
}

async function quitApp() {
  await new Promise((r) => setTimeout(r, 150));
  const ok = await ask("确定退出 xtshell？未断开的会话将逐个关闭。", {
    title: "退出确认",
    kind: "warning",
    okLabel: "退出",
    cancelLabel: "取消",
  });
  if (!ok) return;
  for (const s of [...store.sessions]) {
    try {
      await store.close(s.id);
    } catch {
      /* 已关闭的跳过，继续 */
    }
  }
  await getCurrentWindow().close();
}

onMounted(async () => {
  window.addEventListener("xtshell:new-session", onNewSessionFromMenu);
  await store.refresh();
  unlistenSessions = await onSessionsChanged(() => {
    void store.refresh();
  });
  unlistenProfiles = await onProfilesChanged(() => {
    void profilesStore.refresh();
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("xtshell:new-session", onNewSessionFromMenu);
  unlistenSessions?.();
  unlistenProfiles?.();
  profilesStore.unbind();
});
</script>

<template>
  <div class="app">
    <MenuBar
      @new-session="newSession"
      @disconnect="disconnect"
      @font-settings="showSettings = true"
      @encoding-settings="showEncoding = true"
      @theme-dark="settingsStore.setTheme('dark')"
      @theme-light="settingsStore.setTheme('light')"
      @mcp-info="showMcp = true"
      @sftp="onMenuSftp"
      @copy="menuCopy"
      @paste="menuPaste"
      @reconnect="reconnect"
      @toggle-log="sessionLog.toggleLogging"
      @exit="quitApp"
      @about="showAbout = true"
    />
    <Toolbar
      @new-session="newSession"
      @disconnect="disconnect"
      @reconnect="reconnect"
      @font-settings="showSettings = true"
      @encoding-settings="showEncoding = true"
      @mcp-info="showMcp = true"
    />
    <div class="body">
      <SessionSidebar />
      <div class="terminal-area">
        <TerminalPane @new-session="newSession" />
      </div>
    </div>
    <NewSessionDialog v-model:open="dialogOpen" :edit-profile="profilesStore.editingProfile" />
    <SettingsDialog v-model:open="showSettings" />
    <EncodingDialog v-model:open="showEncoding" />
    <McpDialog v-model:open="showMcp" />
    <AboutDialog v-model:open="showAbout" />
    <ZmodemDialog />
    <ZmodemConflictDialog />
    <ConnectAuthDialog />
    <ContextMenu />
  </div>
</template>

<style>
/* 主题变量：默认浅色（灰色调），深色为可选主题（原配色）。
   store 启动时总会显式设置 data-theme；无属性时按浅色回退。
   组件样式一律引用变量，勿再写死 hex。 */
:root {
  color-scheme: light;
  --bg-app: #f2f2f3;
  --bg-panel: #f9f9fa;
  --bg-input: #ffffff;
  --bg-raised: #e9e9eb;
  --bg-hover: #dcdcdf;
  --bg-hover-soft: #e0e0e2;
  --bg-tabbar: #e4e4e6;
  --bg-sidebar: #ebebed;
  --bg-selected: #d3e2ec;
  --bg-danger-tint: #f6e4e4;
  --border-danger-tint: #e5b8b8;
  --border-subtle: #d5d5d8;
  --border-strong: #bcbcbf;
  --border-divider: #dedee0;
  --border-hover: #c4c4c8;
  --text-primary: #1d1d1f;
  --text-secondary: #3a3a3c;
  --text-body: #2c2c2e;
  --text-muted: #6e6e73;
  --text-faint: #98989d;
  --text-disabled: #b0b0b5;
  --text-on-accent: #ffffff;
  --accent: #0e9488;
  --accent-hover: #12a89a;
  --accent-border: #86c7bf;
  --icon-accent: #0e86a8;
  --accent-blue: #1a7ca8;
  --accent-blue-bright: #0f6286;
  --accent-blue-border: #a3cede;
  --danger: #d93b3b;
  --danger-bright: #e35d5d;
  --danger-border: #eec3c3;
  --warning: #a87b16;
}

:root[data-theme="dark"] {
  color-scheme: dark;
  --bg-app: #1e1e1e;
  --bg-panel: #252525;
  --bg-input: #1a1a1a;
  --bg-raised: #2d2d2d;
  --bg-hover: #3c3c3c;
  --bg-hover-soft: #282828;
  --bg-tabbar: #181818;
  --bg-sidebar: #232323;
  --bg-selected: #2c3a46;
  --bg-danger-tint: #3a1f1f;
  --border-danger-tint: #5a2a2a;
  --border-subtle: #333333;
  --border-strong: #444444;
  --border-divider: #2a2a2a;
  --border-hover: #555555;
  --text-primary: #ffffff;
  --text-secondary: #cccccc;
  --text-body: #e0e0e0;
  --text-muted: #999999;
  --text-faint: #777777;
  --text-disabled: #666666;
  --text-on-accent: #ffffff;
  --accent: #4ec9b0;
  --accent-hover: #5fd8be;
  --accent-border: #2a5a50;
  --icon-accent: #64dafe;
  --accent-blue: #9cdcfe;
  --accent-blue-bright: #c8e6ff;
  --accent-blue-border: #2a5a78;
  --danger: #ff6b6b;
  --danger-bright: #ff8a8a;
  --danger-border: #6a3030;
  --warning: #e5c07b;
}

html,
body,
#app {
  height: 100%;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: var(--bg-app);
}

* {
  box-sizing: border-box;
}
</style>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
}

.body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.terminal-area {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
</style>
