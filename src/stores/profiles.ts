import { defineStore } from "pinia";
import { ref } from "vue";
import {
  listProfiles,
  createFolder,
  renameFolder,
  deleteFolder,
  deleteProfile,
  launchProfile,
  moveProfile,
  moveFolder,
  saveProfile,
} from "../lib/api";
import type { ProfileFolder, ProfileSessionView } from "../types/profile";
import type { Encoding } from "../types/session";
import { useSessionStore } from "./session";

export const useProfilesStore = defineStore("profiles", () => {
  const folders = ref<ProfileFolder[]>([]);
  const profiles = ref<ProfileSessionView[]>([]);
  const expanded = ref<Set<string>>(new Set());
  const error = ref<string | null>(null);
  const editingProfile = ref<ProfileSessionView | null>(null);
  const launchedFrom = ref<Record<string, string>>({});
  // 连接前认证弹框（密码/私钥缺失时）
  const authPromptId = ref<string | null>(null);
  let unlisten: (() => void) | null = null;

  function openEdit(p: ProfileSessionView) {
    editingProfile.value = p;
  }

  /** 判断 profile 连接时是否缺少认证信息 */
  function missingAuth(p: ProfileSessionView): {
    mode: "password" | "key" | "passphrase";
  } | null {
    if (p.protocol !== "ssh") return null;
    if (authMode(p) === "key") {
      if (!p.private_key) return { mode: "key" };
      if (!p.has_passphrase) return { mode: "passphrase" };
      return null;
    }
    if (!p.has_password) return { mode: "password" };
    return null;
  }

  function authMode(p: ProfileSessionView): string {
    return p.auth_mode ?? (p.private_key ? "key" : "password");
  }

  function linkSession(sessionId: string, profileId: string) {
    launchedFrom.value = { ...launchedFrom.value, [sessionId]: profileId };
  }

  function clearEdit() {
    editingProfile.value = null;
  }

  async function refresh() {
    const tree = await listProfiles();
    folders.value = tree.folders;
    profiles.value = tree.sessions;
  }

  function bindEvents(unlistenFn: () => void) {
    unlisten = unlistenFn;
  }

  function unbind() {
    unlisten?.();
    unlisten = null;
  }

  function toggleExpand(id: string) {
    const next = new Set(expanded.value);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expanded.value = next;
  }

  function childrenOf(parent: string | null): ProfileFolder[] {
    return folders.value
      .filter((f) => (f.parent ?? null) === parent)
      .sort((a, b) => a.name.localeCompare(b.name, "zh"));
  }

  function sessionsIn(folder: string | null): ProfileSessionView[] {
    return profiles.value
      .filter((p) => (p.folder ?? null) === folder)
      .sort((a, b) => a.name.localeCompare(b.name, "zh"));
  }

  function fail(e: unknown) {
    error.value = e instanceof Error ? e.message : String(e);
    setTimeout(() => (error.value = null), 4000);
  }

  async function newFolder(name: string, parent: string | null) {
    try {
      await createFolder(name, parent);
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  async function rename(id: string, name: string) {
    try {
      await renameFolder(id, name);
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  async function removeFolder(id: string) {
    try {
      await deleteFolder(id);
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  async function removeProfile(id: string) {
    try {
      await deleteProfile(id);
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  async function launch(id: string) {
    const profile = profiles.value.find((p) => p.id === id);
    // 缺认证信息（密码/私钥/口令未保存）→ 先弹认证框
    const missing = profile ? missingAuth(profile) : null;
    if (missing) {
      authPromptId.value = id;
      return;
    }
    await doLaunch(id, {});
  }

  /** 认证框确认后实际连接 */
  async function doLaunch(
    id: string,
    overrides: { password?: string; passphrase?: string; private_key?: string },
  ) {
    try {
      const sessionId = await launchProfile(id, overrides);
      launchedFrom.value = { ...launchedFrom.value, [sessionId]: id };
      const profile = profiles.value.find((p) => p.id === id);
      if (profile?.encoding) {
        useSessionStore().setEncoding(sessionId, profile.encoding as Encoding);
      }
    } catch (e) {
      fail(e);
    }
  }

  async function renameProfile(id: string, name: string) {
    const p = profiles.value.find((x) => x.id === id);
    if (!p) return;
    try {
      await saveProfile({
        id,
        name,
        folder: p.folder ?? undefined,
        protocol: p.protocol,
        host: p.host,
        port: p.port,
        username: p.username,
        remember_password: true,
        private_key: p.private_key,
        serial_port: p.serial_port,
        baud_rate: p.baud_rate,
        data_bits: p.data_bits,
        stop_bits: p.stop_bits,
        parity: p.parity,
        encoding: p.encoding,
      });
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  async function updateProfileEncoding(profileId: string, encoding: string) {
    const p = profiles.value.find((x) => x.id === profileId);
    if (!p) return;
    try {
      await saveProfile({
        id: profileId,
        name: p.name,
        folder: p.folder ?? undefined,
        protocol: p.protocol,
        host: p.host,
        port: p.port,
        username: p.username,
        remember_password: true,
        private_key: p.private_key,
        serial_port: p.serial_port,
        baud_rate: p.baud_rate,
        data_bits: p.data_bits,
        stop_bits: p.stop_bits,
        parity: p.parity,
        encoding,
      });
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  async function dropProfile(id: string, folder: string | null) {
    try {
      await moveProfile(id, folder);
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  async function dropFolder(id: string, parent: string | null) {
    try {
      await moveFolder(id, parent);
      await refresh();
    } catch (e) {
      fail(e);
    }
  }

  return {
    folders,
    profiles,
    expanded,
    error,
    editingProfile,
    launchedFrom,
    authPromptId,
    openEdit,
    clearEdit,
    linkSession,
    refresh,
    bindEvents,
    unbind,
    toggleExpand,
    childrenOf,
    sessionsIn,
    newFolder,
    rename,
    removeFolder,
    removeProfile,
    launch,
    doLaunch,
    updateProfileEncoding,
    renameProfile,
    dropProfile,
    dropFolder,
  };
});
