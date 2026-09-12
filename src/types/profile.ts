import type { Protocol } from "./session";

export interface ProfileFolder {
  id: string;
  name: string;
  parent: string | null;
}

export interface ProfileSessionView {
  id: string;
  name: string;
  folder: string | null;
  protocol: Protocol;
  host?: string;
  port?: number;
  username?: string;
  private_key?: string;
  auth_mode?: string | null;
  serial_port?: string;
  baud_rate?: number;
  data_bits?: number;
  stop_bits?: number;
  parity?: string;
  encoding?: string;
  created_at: string;
  has_password: boolean;
  has_passphrase: boolean;
}

export interface ProfilesTree {
  folders: ProfileFolder[];
  sessions: ProfileSessionView[];
}

export interface SaveProfileRequest {
  id?: string;
  name: string;
  folder?: string;
  protocol: Protocol;
  auth_mode?: string;
  host?: string;
  port?: number;
  username?: string;
  password?: string;
  remember_password?: boolean;
  private_key?: string;
  private_key_passphrase?: string;
  serial_port?: string;
  baud_rate?: number;
  data_bits?: number;
  stop_bits?: number;
  encoding?: string;
  parity?: string;
}

export interface SerialPortInfo {
  path: string;
  kind: string;
}

// tab 面板类型：预留 SFTP 等非终端面板扩展点
export type PaneKind = "terminal";
