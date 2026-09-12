export interface TerminalActions {
  copy: () => void;
  paste: () => void;
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
