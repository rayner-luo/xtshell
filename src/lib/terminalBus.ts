export interface TerminalActions {
  copy: () => void;
  paste: () => void;
  /** 把键盘焦点交还终端：弹框（按钮被点击后获得焦点）关闭时必须有这一步，
   *  否则焦点落在 body 上，用户敲键盘毫无反应 */
  focus: () => void;
}

const handlers = new Map<string, TerminalActions>();

export function registerTerminalActions(sessionId: string, actions: TerminalActions) {
  handlers.set(sessionId, actions);
}

export function unregisterTerminalActions(sessionId: string) {
  handlers.delete(sessionId);
}

export function getTerminalActions(sessionId: string | null): TerminalActions | undefined {
  return sessionId ? handlers.get(sessionId) : undefined;
}
