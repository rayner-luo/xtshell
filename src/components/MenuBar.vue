<script setup lang="ts">
import { computed, ref, type VNode } from "vue";
import { useSessionStore } from "../stores/session";
import { useSettingsStore } from "../stores/settings";
import { useSessionLog } from "../composables/useSessionLog";
import { AboutIcon, AddIcon, ConnectIcon, CopyIcon, DisconnectIcon, EncodingIcon, ExitIcon, FontIcon, LogIcon, McpIcon, PasteIcon, SftpIcon, ThemeIcon } from "./icons";

type MenuEvent =
  | "new-session"
  | "disconnect"
  | "font-settings"
  | "default-settings"
  | "encoding-settings"
  | "theme-dark"
  | "theme-light"
  | "mcp-info"
  | "sftp"
  | "copy"
  | "paste"
  | "reconnect"
  | "toggle-log"
  | "exit"
  | "about";

interface MenuItem {
  label: string;
  event?: MenuEvent;
  icon?: string;
  iconComponent?: () => VNode;
  shortcut?: string;
  disabled?: boolean;
  /** 激活态图标颜色（与工具栏按钮一致），未配置或禁用时保持灰色 */
  activeColor?: string;
  /** 有 children 时点击展开子菜单而非触发事件 */
  children?: MenuItem[];
  /** 子菜单项勾选标记 */
  checked?: boolean;
}

interface Menu {
  name: string;
  items: MenuItem[];
}

const emit = defineEmits<{
  (e: MenuEvent): void;
}>();

const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const hasActive = computed(() => sessionStore.activeSession != null);
const canReconnect = computed(
  () => sessionStore.activeSession?.state === "closed",
);
const isSsh = computed(() => sessionStore.activeSession?.protocol === "ssh");
const sessionLog = useSessionLog();

const menus = computed<Menu[]>(() => [
  {
    name: "文件",
    items: [
      { label: "新建", event: "new-session", iconComponent: AddIcon, shortcut: "Ctrl+N", activeColor: "var(--icon-accent)" },
      { label: "退出", event: "exit", iconComponent: ExitIcon, activeColor: "var(--icon-accent)" },
    ],
  },
  { name: "编辑", items: [
    { label: "复制", event: "copy", iconComponent: CopyIcon, activeColor: "var(--icon-accent)", disabled: !hasActive.value },
    { label: "粘贴", event: "paste", iconComponent: PasteIcon, activeColor: "var(--icon-accent)", disabled: !hasActive.value },
  ] },
  {
    name: "会话",
    items: [
      { label: "断开", event: "disconnect", iconComponent: DisconnectIcon, activeColor: "var(--icon-accent)", disabled: !hasActive.value },
      { label: "重连", event: "reconnect", iconComponent: ConnectIcon, activeColor: "var(--icon-accent)", disabled: !canReconnect.value },
      { label: "日志", event: "toggle-log", iconComponent: LogIcon, activeColor: sessionLog.isRecording.value ? "var(--danger)" : "var(--icon-accent)", disabled: !hasActive.value },
    ],
  },
  {
    name: "设置",
    items: [
      { label: "字体", event: "font-settings", iconComponent: FontIcon, activeColor: "var(--icon-accent)", disabled: !hasActive.value },
      { label: "编码", event: "encoding-settings", iconComponent: EncodingIcon, activeColor: "var(--icon-accent)", disabled: !hasActive.value },
      {
        label: "主题",
        iconComponent: ThemeIcon,
        children: [
          { label: "深色", event: "theme-dark", checked: settingsStore.theme === "dark" },
          { label: "浅色", event: "theme-light", checked: settingsStore.theme === "light" },
        ],
      },
      { label: "默认设置", event: "default-settings", iconComponent: FontIcon, activeColor: "var(--icon-accent)" },
    ],
  },
  {
    name: "工具",
    items: [
      { label: "SFTP", event: "sftp", iconComponent: SftpIcon, activeColor: "var(--accent)", disabled: !isSsh.value },
      { label: "MCP", event: "mcp-info", iconComponent: McpIcon, activeColor: "var(--accent-blue)" },
    ],
  },
  { name: "帮助", items: [{ label: "关于", event: "about", iconComponent: AboutIcon, activeColor: "var(--icon-accent)" }] },
]);

function iconStyle(item: MenuItem) {
  if (item.disabled || !item.activeColor) return undefined;
  return { color: item.activeColor };
}

const openMenu = ref<string | null>(null);
const openSubmenu = ref<string | null>(null);

function toggleMenu(name: string) {
  openMenu.value = openMenu.value === name ? null : name;
  openSubmenu.value = null;
}

function closeMenu() {
  openMenu.value = null;
  openSubmenu.value = null;
}

function toggleSubmenu(label: string) {
  openSubmenu.value = openSubmenu.value === label ? null : label;
}

function onItemClick(item: MenuItem) {
  if (item.children) {
    toggleSubmenu(item.label);
    return;
  }
  closeMenu();
  if (item.event) emit(item.event);
}
</script>

<template>
  <div class="menu-bar" @mouseleave="closeMenu">
    <div v-for="menu in menus" :key="menu.name" class="menu">
      <button
        class="menu-title"
        :class="{ active: openMenu === menu.name }"
        @click="toggleMenu(menu.name)"
      >
        {{ menu.name }}
      </button>
      <div v-if="openMenu === menu.name" class="dropdown">
        <div v-for="item in menu.items" :key="item.label" class="dropdown-entry">
          <button
            class="dropdown-item"
            :class="{ expanded: item.children && openSubmenu === item.label }"
            :disabled="item.disabled"
            @click="onItemClick(item)"
          >
            <span v-if="item.iconComponent" class="item-icon" :style="iconStyle(item)">
              <component :is="item.iconComponent" />
            </span>
            <span v-else-if="item.icon" class="item-icon">{{ item.icon }}</span>
            <span class="item-label">{{ item.label }}</span>
            <span v-if="item.shortcut" class="item-shortcut">{{ item.shortcut }}</span>
            <span v-if="item.children" class="item-arrow">▸</span>
          </button>
          <div v-if="item.children && openSubmenu === item.label" class="submenu">
            <button
              v-for="child in item.children"
              :key="child.label"
              class="dropdown-item"
              :disabled="child.disabled"
              @click="onItemClick(child)"
            >
              <span class="item-check">{{ child.checked ? "✓" : "" }}</span>
              <span class="item-label">{{ child.label }}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.menu-bar {
  display: flex;
  align-items: stretch;
  height: 32px;
  background: var(--bg-raised);
  border-bottom: 1px solid var(--bg-input);
  user-select: none;
  flex-shrink: 0;
}

.menu {
  position: relative;
  display: flex;
}

.menu-title {
  padding: 0 12px;
  font-size: 12px;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  cursor: pointer;
}

.menu-title:hover,
.menu-title.active {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dropdown {
  position: absolute;
  top: 32px;
  left: 0;
  min-width: 180px;
  background: var(--bg-raised);
  border: 1px solid var(--border-strong);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  z-index: 100;
  display: flex;
  flex-direction: column;
  padding: 4px 0;
}

.dropdown-entry {
  position: relative;
  display: flex;
  flex-direction: column;
}

.dropdown-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  font-size: 12px;
  text-align: left;
  color: var(--text-secondary);
  background: transparent;
  border: none;
  cursor: pointer;
  white-space: nowrap;
}

.submenu {
  position: absolute;
  top: -4px;
  left: calc(100% - 2px);
  min-width: 140px;
  background: var(--bg-raised);
  border: 1px solid var(--border-strong);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  padding: 4px 0;
  z-index: 101;
}

.item-icon {
  width: 22px;
  flex-shrink: 0;
  text-align: center;
  color: var(--text-muted);
}

.item-label {
  flex: 1;
}

.item-shortcut {
  margin-left: 24px;
  color: var(--text-faint);
  font-size: 11px;
}

.item-arrow {
  margin-left: 12px;
  color: var(--text-faint);
  font-size: 11px;
}

.item-check {
  width: 14px;
  flex-shrink: 0;
  color: var(--accent);
  font-size: 11px;
}

.dropdown-item:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dropdown-item.expanded:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.dropdown-item:disabled {
  color: var(--text-disabled);
  cursor: default;
}
</style>
