import { defineStore } from "pinia";
import { ref } from "vue";
import type { ZmodemConflict } from "../lib/api";

export const useZmodemStore = defineStore("zmodem", () => {
  // 三选兜底（无 hint 时）
  const pendingId = ref<string | null>(null);
  // 同名冲突裁决队列（逐个弹）
  const conflicts = ref<(ZmodemConflict & { sessionId: string })[]>([]);

  function request(id: string) {
    pendingId.value = id;
  }

  function resolve() {
    pendingId.value = null;
  }

  function addConflict(sessionId: string, info: ZmodemConflict) {
    conflicts.value.push({ ...info, sessionId });
  }

  function shiftConflict() {
    return conflicts.value.shift();
  }

  return { pendingId, resolve, request, conflicts, addConflict, shiftConflict };
});
