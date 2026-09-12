import { reactive } from "vue";

export interface MenuItem {
  label: string;
  danger?: boolean;
  action: () => void;
}

interface CtxMenuState {
  x: number;
  y: number;
  items: MenuItem[] | null;
}

export const ctxMenu = reactive<CtxMenuState>({ x: 0, y: 0, items: null });

export function openCtxMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  ctxMenu.x = e.clientX;
  ctxMenu.y = e.clientY;
  ctxMenu.items = items;
}

export function closeCtxMenu() {
  ctxMenu.items = null;
}
