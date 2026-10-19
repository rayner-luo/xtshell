import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SessionConfig, SessionInfo } from "../types/session";
import type { ProfilesTree, SaveProfileRequest, SerialPortInfo } from "../types/profile";
import type { TerminalAppearance } from "../types/profile";

export const createSession = (config: SessionConfig) =>
  invoke<string>("create_session", { config });
export const closeSession = (id: string) => invoke<void>("close_session", { id });
export const reconnectSession = (id: string) =>
  invoke<string>("reconnect_session", { id });
export const listSessions = () => invoke<SessionInfo[]>("list_sessions");
export const sendInput = (id: string, data: string) =>
  invoke<void>("send_input", { id, data });
export const resizeSession = (id: string, cols: number, rows: number) =>
  invoke<void>("resize_session", { id, cols, rows });
export const getSessionOutput = (id: string, offset: number) =>
  invoke<[number[], number]>("get_session_output", { id, offset });

/** Linux 上同步 GTK 原生主题（select 下拉、右键菜单等原生弹出跟随应用主题） */
export const setNativeTheme = (dark: boolean) =>
  invoke<void>("set_native_theme", { dark });

export function onSessionOutput(
  id: string,
  cb: (data: Uint8Array) => void,
): Promise<UnlistenFn> {
  return listen<number[]>(`session-output-${id}`, (e) => {
    cb(new Uint8Array(e.payload));
  });
}

export function onSessionClosed(id: string, cb: () => void): Promise<UnlistenFn> {
  return listen<string>(`session-state-${id}`, () => cb());
}

export function onSessionError(
  id: string,
  cb: (message: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(`session-error-${id}`, (e) => cb(e.payload));
}

/** 使用档案中已保存的凭据认证失败：前端据此弹出凭据重输框 */
export interface SessionAuthFailed {
  session_id: string;
  profile_id: string;
  message: string;
}

export function onSessionAuthFailed(
  id: string,
  cb: (p: SessionAuthFailed) => void,
): Promise<UnlistenFn> {
  return listen<SessionAuthFailed>(`session-auth-failed-${id}`, (e) =>
    cb(e.payload),
  );
}

export function onSessionsChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("sessions-changed", () => cb());
}

export const listProfiles = () => invoke<ProfilesTree>("list_profiles");
export const saveProfile = (req: SaveProfileRequest) =>
  invoke<string>("save_profile", { req });
export const updateProfileAppearance = (id: string, appearance: TerminalAppearance) =>
  invoke<void>("update_profile_appearance", { id, appearance });
export const deleteProfile = (id: string) =>
  invoke<void>("delete_profile", { id });
export const launchProfile = (
  id: string,
  overrides?: { password?: string; passphrase?: string; private_key?: string },
) =>
  invoke<string>("launch_profile", {
    id,
    password: overrides?.password ?? null,
    passphrase: overrides?.passphrase ?? null,
    private_key: overrides?.private_key ?? null,
  });
export const setProfileSecret = (
  id: string,
  secrets: { password?: string; passphrase?: string; private_key?: string },
) =>
  invoke<void>("set_profile_secret", {
    id,
    password: secrets.password ?? null,
    passphrase: secrets.passphrase ?? null,
    private_key: secrets.private_key ?? null,
  });
export const createFolder = (name: string, parent: string | null) =>
  invoke<string>("create_folder", { name, parent });
export const renameFolder = (id: string, name: string) =>
  invoke<void>("rename_folder", { id, name });
export const deleteFolder = (id: string) => invoke<void>("delete_folder", { id });
export const moveProfile = (id: string, folder: string | null) =>
  invoke<void>("move_profile", { id, folder });
export const moveFolder = (id: string, parent: string | null) =>
  invoke<void>("move_folder", { id, parent });
export const listSerialPorts = () =>
  invoke<SerialPortInfo[]>("list_serial_ports");
export const setSessionLog = (id: string, path: string | null) =>
  invoke<string | null>("set_session_log", { id, path });

export function onProfilesChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("profiles-changed", () => cb());
}

// SSH 主机密钥（首次连接确认 / 变更告警）
export interface SshHostKeyPrompt {
  session_id: string;
  host: string;
  port: number;
  key_type: string;
  fingerprint: string;
  known_hosts: string;
}

export const sshHostKeyDecision = (id: string, accept: boolean) =>
  invoke<void>("ssh_hostkey_decision", { id, accept });

export function onSshHostKeyPrompt(
  cb: (p: SshHostKeyPrompt) => void,
): Promise<UnlistenFn> {
  return listen<SshHostKeyPrompt>("ssh-hostkey-prompt", (e) => cb(e.payload));
}

// ZMODEM（lrzsz）
export interface ZmodemProgress {
  kind: string;
  name: string;
  done: number;
  total: number;
}
export interface ZmodemConflict {
  temp: string;
  name: string;
  dest: string;
  size: number;
}

export const zmodemStartReceive = (id: string, dir: string) =>
  invoke<void>("zmodem_start_receive", { id, dir });
export const zmodemSendFiles = (id: string, paths: string[]) =>
  invoke<void>("zmodem_send_files", { id, paths });
export const zmodemResolveConflict = (
  temp: string,
  dest: string,
  choice: string,
) => invoke<void>("zmodem_resolve_conflict", { temp, dest, choice });
export const zmodemCancel = (id: string) =>
  invoke<void>("zmodem_cancel", { id });

export function onZmodemAskDir(id: string, cb: () => void): Promise<UnlistenFn> {
  return listen(`zmodem-ask-dir-${id}`, () => cb());
}
export function onZmodemAskFiles(id: string, cb: () => void): Promise<UnlistenFn> {
  return listen(`zmodem-ask-files-${id}`, () => cb());
}
export function onZmodemRequest(id: string, cb: () => void): Promise<UnlistenFn> {
  return listen(`zmodem-request-${id}`, () => cb());
}
export function onZmodemConflict(
  id: string,
  cb: (info: ZmodemConflict) => void,
): Promise<UnlistenFn> {
  return listen<ZmodemConflict>(`zmodem-conflict-${id}`, (e) => cb(e.payload));
}

// SFTP
export interface SftpEntry {
  name: string;
  is_dir: boolean;
  size: number;
  mtime: number;
}
export interface SftpProgress {
  kind: string;
  name: string;
  done: number;
  total: number;
}

export const sftpConnect = (id: string, path?: string) =>
  invoke<[string, SftpEntry[]]>("sftp_connect", { id, path: path ?? null });
export const sftpDisconnect = (id: string) =>
  invoke<void>("sftp_disconnect", { id });
export const sftpList = (id: string, path: string) =>
  invoke<SftpEntry[]>("sftp_list", { id, path });
export const sftpMkdir = (id: string, path: string) =>
  invoke<void>("sftp_mkdir", { id, path });
export const sftpRemove = (id: string, path: string, isDir: boolean) =>
  invoke<void>("sftp_remove", { id, path, isDir });
export const sftpRename = (id: string, oldPath: string, newPath: string) =>
  invoke<void>("sftp_rename", { id, oldPath, newPath });
export const sftpCheckUpload = (id: string, remoteDir: string, names: string[]) =>
  invoke<string[]>("sftp_check_upload", { id, remoteDir, names });
export const sftpUpload = (
  id: string,
  remoteDir: string,
  localPaths: string[],
  policy: string,
) => invoke<[number, number]>("sftp_upload", { id, remoteDir, localPaths, policy });
export const sftpDownload = (id: string, remote: string, local: string) =>
  invoke<number>("sftp_download", { id, remote, local });

export function onSftpProgress(
  id: string,
  cb: (p: SftpProgress) => void,
): Promise<UnlistenFn> {
  return listen<SftpProgress>(`sftp-progress-${id}`, (e) => cb(e.payload));
}
export function onZmodemProgress(
  id: string,
  cb: (p: ZmodemProgress) => void,
): Promise<UnlistenFn> {
  return listen<ZmodemProgress>(`zmodem-progress-${id}`, (e) => cb(e.payload));
}
