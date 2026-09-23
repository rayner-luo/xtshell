<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useSessionStore } from "../stores/session";
import { useProfilesStore } from "../stores/profiles";
import { openCtxMenu } from "../composables/contextMenu";
import SidebarTreeNode from "./SidebarTreeNode.vue";
import SftpPanel from "./SftpPanel.vue";

const sessionStore = useSessionStore();
const profilesStore = useProfilesStore();

const collapsed = computed(() => sessionStore.sidebarCollapsed);
// 仅 SSH 会话可启用 SFTP 页签
const sftpEnabled = computed(
  () => sessionStore.activeSession?.protocol === "ssh",
);
const panelTabs = computed(() => [
  { id: "sessions" as const, label: "会话", enabled: true },
  { id: "sftp" as const, label: "SFTP", enabled: sftpEnabled.value },
]);

// 面板状态放 session store（工具栏 SFTP 按钮可远程切换）
const activePanel = computed<"sessions" | "sftp">(() =>
  sessionStore.sftpPanelActive ? "sftp" : "sessions",
);

function switchPanel(id: "sessions" | "sftp") {
  if (id === "sftp" && !sftpEnabled.value) return;
  sessionStore.sftpPanelActive = id === "sftp";
}
const rootFolders = computed(() => profilesStore.childrenOf(null));
const rootSessions = computed(() => profilesStore.sessionsIn(null));

onMounted(async () => {
  await profilesStore.refresh();
});


function sidebarMenu(e: MouseEvent) {
  openCtxMenu(e, [
    { label: "新建连接", action: () => profilesStore.openNew(null) },
    {
      label: "新建目录",
      action: () => profilesStore.newFolder("新建目录", null),
    },
    { label: "刷新", action: () => profilesStore.refresh() },
  ]);
}

async function onRootDrop(e: DragEvent) {
  e.preventDefault();
  const folderId = e.dataTransfer!.getData("xtshell/folder");
  if (folderId) {
    await profilesStore.dropFolder(folderId, null);
    return;
  }
  const profileId = e.dataTransfer!.getData("xtshell/profile");
  if (profileId) await profilesStore.dropProfile(profileId, null);
}
</script>

<template>
  <div class="sidebar" :class="{ collapsed }">
    <template v-if="!collapsed">
      <div class="panel-strip">
        <button
          v-for="tab in panelTabs"
          :key="tab.id"
          class="panel-tab"
          :class="{ active: tab.enabled && activePanel === tab.id, disabled: !tab.enabled }"
          :disabled="!tab.enabled"
          @click="switchPanel(tab.id)"
        >
          {{ tab.label }}
        </button>
        <span class="spacer"></span>
        <button class="collapse-btn" title="收起侧栏" @click="sessionStore.toggleSidebar()">
          «
        </button>
      </div>

      <div class="panel-body">
        <template v-if="activePanel === 'sessions'">
        <section class="sec grow">
          <div
            class="tree-root"
            @dragover.prevent
            @drop="onRootDrop"
            @contextmenu="sidebarMenu"
          >
            <div v-if="rootFolders.length === 0 && rootSessions.length === 0" class="sec-empty">
              右键新建目录，或在新建会话时选择保存
            </div>
            <SidebarTreeNode
              v-for="f in rootFolders"
              :key="f.id"
              kind="folder"
              :folder="f"
              :depth="0"
            />
            <SidebarTreeNode
              v-for="p in rootSessions"
              :key="p.id"
              kind="profile"
              :profile="p"
              :depth="0"
            />
          </div>
        </section>

        <div v-if="profilesStore.error" class="sidebar-error">{{ profilesStore.error }}</div>
        </template>
        <SftpPanel v-else-if="activePanel === 'sftp'" />
      </div>
    </template>

    <div v-else class="collapsed-strip" @click="sessionStore.toggleSidebar()" title="展开侧栏">
      <span class="vtext head">会话</span>
    </div>
  </div>
</template>

<style scoped>
.sidebar {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border-divider);
  overflow: hidden;
  transition: width 0.15s ease;
}

.sidebar.collapsed {
  width: 24px;
}

.panel-strip {
  display: flex;
  align-items: center;
  height: 28px;
  flex-shrink: 0;
  background: var(--bg-tabbar);
  border-bottom: 1px solid var(--border-divider);
}

.panel-tab {
  padding: 0 12px;
  height: 100%;
  font-size: 12px;
  color: var(--text-muted);
  background: transparent;
  border: none;
  cursor: pointer;
}

.panel-tab.active {
  color: var(--text-primary);
  background: var(--bg-sidebar);
  box-shadow: inset 0 -2px 0 var(--accent);
}

.panel-tab:disabled {
  opacity: 0.4;
  cursor: default;
}

.collapse-btn {
  margin-left: auto;
  width: 24px;
  height: 100%;
  font-size: 12px;
  color: var(--text-muted);
  background: transparent;
  border: none;
  cursor: pointer;
}

.collapse-btn:hover {
  color: var(--text-primary);
  background: var(--border-subtle);
}

.panel-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.sec {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 6px 0;
}

.sec.grow {
  flex: 1;
  border-top: 1px solid var(--border-divider);
}

.sec-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 10px 6px;
  font-size: 11px;
  color: var(--text-faint);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.sec-empty {
  padding: 4px 10px;
  font-size: 11px;
  color: var(--text-disabled);
}

.tree-root {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-bottom: 6px;
  scrollbar-width: thin;
  scrollbar-color: rgba(121, 121, 121, 0.4) transparent;
}

/* VS Code 风格细滚动条（与终端 xterm 滚动条一致） */
.tree-root::-webkit-scrollbar {
  width: 10px;
}

.tree-root::-webkit-scrollbar-track {
  background: transparent;
}

.tree-root::-webkit-scrollbar-thumb {
  background: rgba(121, 121, 121, 0.4);
  border-radius: 5px;
}

.tree-root::-webkit-scrollbar-thumb:hover {
  background: rgba(100, 100, 100, 0.7);
}

.sidebar-error {
  padding: 4px 10px;
  font-size: 11px;
  color: var(--danger-bright);
  background: var(--bg-danger-tint);
  flex-shrink: 0;
}

.collapsed-strip {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 10px 0;
  width: 100%;
  height: 100%;
  cursor: pointer;
  background: var(--bg-sidebar);
}

.collapsed-strip:hover {
  background: var(--bg-hover-soft);
}

.vtext {
  writing-mode: vertical-rl;
  font-size: 12px;
  color: var(--text-muted);
  white-space: nowrap;
  max-height: 130px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.vtext.head {
  color: var(--accent);
  font-weight: 600;
}

</style>
