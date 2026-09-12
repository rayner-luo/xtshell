<script setup lang="ts">
import { onMounted, onBeforeUnmount } from "vue";
import { ctxMenu, closeCtxMenu } from "../composables/contextMenu";

function onAction() {
  closeCtxMenu();
}

onMounted(() => window.addEventListener("mousedown", onWindowMousedown));
onBeforeUnmount(() => window.removeEventListener("mousedown", onWindowMousedown));

function onWindowMousedown() {
  if (ctxMenu.items) closeCtxMenu();
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="ctxMenu.items"
      class="ctx-menu"
      :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }"
      @mousedown.stop
    >
      <button
        v-for="item in ctxMenu.items"
        :key="item.label"
        class="ctx-item"
        :class="{ danger: item.danger }"
        @click.stop="item.action(), onAction()"
      >
        {{ item.label }}
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.ctx-menu {
  position: fixed;
  min-width: 130px;
  padding: 4px 0;
  background: #2a2a2a;
  border: 1px solid #444444;
  border-radius: 6px;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.55);
  z-index: 2000;
}

.ctx-item {
  display: block;
  width: 100%;
  padding: 6px 16px;
  font-size: 12px;
  text-align: left;
  color: #cccccc;
  background: transparent;
  border: none;
  cursor: pointer;
}

.ctx-item:hover {
  background: #3c3c3c;
  color: #ffffff;
}

.ctx-item.danger {
  color: #ff8a8a;
}

.ctx-item.danger:hover {
  background: #4a2626;
  color: #ffaaaa;
}
</style>
