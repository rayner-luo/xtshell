<script setup lang="ts">
import { computed, ref, type Directive } from "vue";
import { useProfilesStore } from "../stores/profiles";
import { openCtxMenu } from "../composables/contextMenu";
import { FolderIcon } from "./icons";
import type { ProfileFolder, ProfileSessionView } from "../types/profile";

const props = defineProps<{
  kind: "folder" | "profile";
  folder?: ProfileFolder;
  profile?: ProfileSessionView;
  depth: number;
}>();

const vFocus: Directive<HTMLInputElement> = {
  mounted(el) {
    el.focus();
    el.select();
  },
};

const profilesStore = useProfilesStore();
const editing = ref(false);
const editName = ref("");
const dragOver = ref(false);

const childFolders = computed(() =>
  props.kind === "folder" ? profilesStore.childrenOf(props.folder!.id) : [],
);
const childSessions = computed(() =>
  props.kind === "folder" ? profilesStore.sessionsIn(props.folder!.id) : [],
);
const expanded = computed(() =>
  props.kind === "folder" ? profilesStore.expanded.has(props.folder!.id) : false,
);
const indent = computed(() => 8 + props.depth * 14);
// 会话节点补齐目录 caret 的等宽占位，使子会话明显缩进于父目录之下
const profileIndent = computed(() => indent.value + (props.depth > 0 ? 14 : 0));

const protoTag = computed(() =>
  props.profile ? { ssh: "SSH", telnet: "TEL", serial: "SER" }[props.profile.protocol] : "",
);

function toggle() {
  if (props.kind === "folder") profilesStore.toggleExpand(props.folder!.id);
}

function startEdit() {
  editName.value = props.kind === "folder" ? props.folder!.name : props.profile!.name;
  editing.value = true;
}

async function commitEdit() {
  const name = editName.value.trim();
  if (!editing.value || !name) return;
  editing.value = false;
  if (props.kind === "folder") {
    await profilesStore.rename(props.folder!.id, name);
  } else {
    await profilesStore.renameProfile(props.profile!.id, name);
  }
}

function cancelEdit() {
  editing.value = false;
}

function folderMenu(e: MouseEvent) {
  openCtxMenu(e, [
    {
      label: "新建子目录",
      action: () => profilesStore.newFolder("新建目录", props.folder!.id),
    },
    { label: "重命名", action: startEdit },
    { label: "删除", danger: true, action: () => profilesStore.removeFolder(props.folder!.id) },
  ]);
}

function profileMenu(e: MouseEvent) {
  openCtxMenu(e, [
    { label: "连接", action: () => profilesStore.launch(props.profile!.id) },
    { label: "编辑", action: () => profilesStore.openEdit(props.profile!) },
    { label: "重命名", action: startEdit },
    {
      label: "删除",
      danger: true,
      action: () => profilesStore.removeProfile(props.profile!.id),
    },
  ]);
}

function onDragStart(e: DragEvent) {
  if (props.kind === "folder") {
    e.dataTransfer!.setData("xtshell/folder", props.folder!.id);
  } else {
    e.dataTransfer!.setData("xtshell/profile", props.profile!.id);
  }
  e.dataTransfer!.effectAllowed = "move";
}

function onDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  dragOver.value = false;
  const folderId = props.folder!.id;
  const profileId = e.dataTransfer!.getData("xtshell/profile");
  if (profileId) {
    void profilesStore.dropProfile(profileId, folderId);
    return;
  }
  const droppedFolder = e.dataTransfer!.getData("xtshell/folder");
  if (droppedFolder && droppedFolder !== folderId) {
    void profilesStore.dropFolder(droppedFolder, folderId);
  }
}
</script>

<template>
  <div v-if="kind === 'folder'">
    <div
      class="node folder"
      :class="{ dropTarget: dragOver }"
      :style="{ paddingLeft: indent + 'px' }"
      draggable="true"
      @dragstart="onDragStart"
      @dragover.prevent="dragOver = true"
      @dragleave="dragOver = false"
      @drop="onDrop"
      @contextmenu="folderMenu"
    >
      <span class="caret" @click.stop="toggle">{{ expanded ? "▾" : "▸" }}</span>
      <template v-if="!editing">
        <component class="icon" :is="FolderIcon" />
        <span class="name" @dblclick="startEdit">{{ folder!.name }}</span>
        <span class="count">{{ childFolders.length + childSessions.length || "" }}</span>
      </template>
      <input
        v-else
        v-model="editName"
        class="edit-input"
        @keydown.enter="commitEdit"
        @keydown.esc="cancelEdit"
        @blur="commitEdit"
        v-focus
      />
    </div>
    <div v-if="expanded" class="children">
      <SidebarTreeNode
        v-for="f in childFolders"
        :key="f.id"
        kind="folder"
        :folder="f"
        :depth="depth + 1"
      />
      <SidebarTreeNode
        v-for="p in childSessions"
        :key="p.id"
        kind="profile"
        :profile="p"
        :depth="depth + 1"
      />
    </div>
  </div>

  <div
    v-else
    class="node profile"
    :style="{ paddingLeft: profileIndent + 'px' }"
    draggable="true"
    @dragstart="onDragStart"
    @contextmenu="profileMenu"
    @dblclick="profilesStore.launch(props.profile!.id)"
  >
    <span class="proto-badge" :class="profile!.protocol">{{ protoTag }}</span>
    <template v-if="!editing">
      <span class="name">{{ profile!.name }}</span>
    </template>
    <input
      v-else
      v-model="editName"
      class="edit-input"
      @keydown.enter="commitEdit"
      @keydown.esc="cancelEdit"
      @blur="commitEdit"
      v-focus
    />
  </div>
</template>

<style scoped>
.node {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  font-size: 12px;
  color: #bbbbbb;
  cursor: default;
  white-space: nowrap;
  user-select: none;
}

.node:hover {
  background: #2a2a2a;
}

.node.dropTarget {
  background: #2c3a46;
  outline: 1px dashed #4ec9b0;
}

.caret {
  width: 14px;
  height: 26px;
  line-height: 26px;
  text-align: center;
  color: #888888;
  cursor: pointer;
  flex-shrink: 0;
}

.icon {
  font-size: 12px;
  color: #64dafe;
}

.name {
  overflow: hidden;
  text-overflow: ellipsis;
}

.count {
  font-size: 10px;
  color: #666666;
}

.proto-badge {
  flex-shrink: 0;
  font-size: 9px;
  font-weight: 700;
  line-height: 14px;
  padding: 0 4px;
  border-radius: 3px;
}

.proto-badge.ssh {
  color: #4ec9b0;
}

.proto-badge.telnet {
  color: #e5c07b;
}

.proto-badge.serial {
  color: #56b6c2;
}

.edit-input {
  width: 120px;
  padding: 2px 6px;
  font-size: 12px;
  color: #e0e0e0;
  background: #1a1a1a;
  border: 1px solid #4ec9b0;
  border-radius: 3px;
  outline: none;
}
</style>
