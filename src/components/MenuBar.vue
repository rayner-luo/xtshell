<script setup lang="ts">
import { computed, ref } from "vue";
import { useSessionStore } from "../stores/session";

type MenuEvent =
  | "new-session"
  | "disconnect"
  | "font-settings"
  | "encoding-settings"
  | "mcp-info"
  | "sftp"
  | "copy"
  | "paste"
  | "reconnect"
  | "exit"
  | "about";

interface MenuItem {
  label: string;
  event?: MenuEvent;
  icon?: string;
  shortcut?: string;
  disabled?: boolean;
}

interface Menu {
  name: string;
  items: MenuItem[];
}

const emit = defineEmits<{
  (e: MenuEvent): void;
}>();

const sessionStore = useSessionStore();
const hasActive = computed(() => sessionStore.activeSession != null);
const canReconnect = computed(
  () => sessionStore.activeSession?.state === "closed",
);

const menus = computed<Menu[]>(() => [
  {
    name: "文件",
    items: [
      { label: "新建", event: "new-session", icon: "➕", shortcut: "Ctrl+N" },
      { label: "退出", event: "exit", icon: "✕" },
    ],
  },
  { name: "编辑", items: [
    { label: "复制", event: "copy", icon: "⧉", disabled: !hasActive.value },
    { label: "粘贴", event: "paste", icon: "📋", disabled: !hasActive.value },
  ] },
  {
    name: "会话",
    items: [
      { label: "断开", event: "disconnect", icon: "⛓️‍💥", disabled: !hasActive.value },
      { label: "重连", event: "reconnect", icon: "🔗", disabled: !canReconnect.value },
    ],
  },
  {
    name: "设置",
    items: [
      { label: "字体", event: "font-settings", icon: "Aa" },
      { label: "编码", event: "encoding-settings", icon: "🌐" },
    ],
  },
  {
    name: "工具",
    items: [
      { label: "SFTP", event: "sftp", icon: "⇄" },
      { label: "MCP", event: "mcp-info", icon: "〽️" },
    ],
  },
  { name: "帮助", items: [{ label: "关于", event: "about", icon: "ℹ" }] },
]);

const openMenu = ref<string | null>(null);

function toggleMenu(name: string) {
  openMenu.value = openMenu.value === name ? null : name;
}

function closeMenu() {
  openMenu.value = null;
}

function onItemClick(item: MenuItem) {
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
        <button
          v-for="item in menu.items"
          :key="item.label"
          class="dropdown-item"
          :disabled="item.disabled"
          @click="onItemClick(item)"
        >
          <span v-if="item.icon" class="item-icon">{{ item.icon }}</span>
          <span class="item-label">{{ item.label }}</span>
          <span v-if="item.shortcut" class="item-shortcut">{{ item.shortcut }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.menu-bar {
  display: flex;
  align-items: stretch;
  height: 32px;
  background: #2d2d2d;
  border-bottom: 1px solid #1a1a1a;
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
  color: #cccccc;
  background: transparent;
  border: none;
  cursor: pointer;
}

.menu-title:hover,
.menu-title.active {
  background: #3c3c3c;
  color: #ffffff;
}

.dropdown {
  position: absolute;
  top: 32px;
  left: 0;
  min-width: 180px;
  background: #2d2d2d;
  border: 1px solid #444444;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
  z-index: 100;
  display: flex;
  flex-direction: column;
  padding: 4px 0;
}

.dropdown-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  font-size: 12px;
  text-align: left;
  color: #cccccc;
  background: transparent;
  border: none;
  cursor: pointer;
  white-space: nowrap;
}

.item-icon {
  width: 22px;
  flex-shrink: 0;
  text-align: center;
  color: #999999;
}

.item-label {
  flex: 1;
}

.item-shortcut {
  margin-left: 24px;
  color: #777777;
  font-size: 11px;
}

.dropdown-item:hover:not(:disabled) {
  background: #3c3c3c;
  color: #ffffff;
}

.dropdown-item:disabled {
  color: #666666;
  cursor: default;
}
</style>
