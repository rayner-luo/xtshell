import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { ZmodemConflict } from "../lib/api";

/** send = 本端上传（远端 rz）；receive = 本端下载（远端 sz） */
export type ZmodemIntent = "send" | "receive";

export const useZmodemStore = defineStore("zmodem", () => {
  // 三选兜底（无 hint 时）
  const pendingId = ref<string | null>(null);
  // 待取路径队列：后端 ask-dir/ask-files 或三选兜底转入，由 ZmodemPickDialog 消费
  const picks = ref<{ sessionId: string; intent: ZmodemIntent }[]>([]);
  const pick = computed(() => picks.value[0] ?? null);
  // 同名冲突裁决队列（逐个弹）
  const conflicts = ref<(ZmodemConflict & { sessionId: string })[]>([]);

  function request(id: string) {
    pendingId.value = id;
  }

  function resolve() {
    pendingId.value = null;
  }

  /** 请求用户给出文件/目录路径（同会话已有待决请求时忽略重复） */
  function askPick(sessionId: string, intent: ZmodemIntent) {
    if (picks.value.some((p) => p.sessionId === sessionId)) return;
    picks.value = [...picks.value, { sessionId, intent }];
  }

  function resolvePick() {
    picks.value = picks.value.slice(1);
  }

  function addConflict(sessionId: string, info: ZmodemConflict) {
    conflicts.value.push({ ...info, sessionId });
  }

  function shiftConflict() {
    return conflicts.value.shift();
  }

  return {
    pendingId,
    resolve,
    request,
    pick,
    askPick,
    resolvePick,
    conflicts,
    addConflict,
    shiftConflict,
  };
});
