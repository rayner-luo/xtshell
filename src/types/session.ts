export type Protocol = "ssh" | "telnet" | "serial";
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
}
