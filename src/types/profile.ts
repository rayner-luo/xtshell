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
  /** 本地 Shell（仅 local 生效） */
  shell?: string;
  /** 本地 Shell 启动参数（仅 local 且 shell 非空时生效） */
  shell_args?: string;
  encoding?: string;
  terminal_appearance?: TerminalAppearance | null;
  forwards?: PortForward[];
  socks5?: Socks5Config | null;
  created_at: string;
  has_password: boolean;
  has_passphrase: boolean;
}

/** SSH 动态代理（SOCKS5）：仅监听 127.0.0.1，无认证 */
export interface Socks5Config {
  enabled: boolean;
  local_port: number;
}

/** SSH 本地端口转发：本地监听 → 经会话转发到远端 */
export interface PortForward {
  local_addr: string;
  local_port: number;
  remote_addr: string;
  remote_port: number;
}

export interface TerminalAppearance {
  font_family?: string | null;
  font_size?: number | null;
  encoding?: string | null;
  color_scheme?: string | null;
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
  terminal_appearance?: TerminalAppearance | null;
  forwards?: PortForward[];
  socks5?: Socks5Config | null;
  parity?: string;
  /** 本地 Shell（仅 local 生效） */
  shell?: string;
  /** 本地 Shell 启动参数（仅 local 且 shell 非空时生效） */
  shell_args?: string;
}

export interface SerialPortInfo {
  path: string;
  kind: string;
}

// tab 面板类型：预留 SFTP 等非终端面板扩展点
export type PaneKind = "terminal";
