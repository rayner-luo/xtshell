import type { PortForward, Socks5Config } from "./profile";

export type Protocol = "ssh" | "telnet" | "serial" | "local";
export type SessionState = "connecting" | "connected" | "closed";

export type Encoding = "utf-8" | "gbk" | "gb18030" | "big5" | "latin1";

export const encodingOptions: { value: Encoding; label: string }[] = [
  { value: "utf-8", label: "UTF-8" },
  { value: "gbk", label: "GBK" },
  { value: "gb18030", label: "GB18030" },
  { value: "big5", label: "Big5" },
  { value: "latin1", label: "Latin-1" },
];

export interface SessionInfo {
  id: string;
  protocol: Protocol;
  target: string;
  state: SessionState;
}

export interface SessionConfig {
  protocol: Protocol;
  host?: string;
  port?: number;
  username?: string;
  password?: string;
  private_key?: string;
  private_key_passphrase?: string;
  serial_port?: string;
  baud_rate?: number;
  data_bits?: number;
  stop_bits?: number;
  parity?: string;
  /** 本地 Shell（仅 local 生效）：可执行文件路径；留空使用系统默认 */
  shell?: string;
  /** 本地 Shell 启动参数（仅 local 且 shell 非空时生效） */
  shell_args?: string;
  /** SSH 本地端口转发（仅 ssh 生效） */
  forwards?: PortForward[];
  /** SSH SOCKS5 动态代理（仅 ssh 生效） */
  socks5?: Socks5Config | null;
}
