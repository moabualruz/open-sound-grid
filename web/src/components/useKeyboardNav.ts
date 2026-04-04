import { createSignal } from "solid-js";
import type { MatrixCellActions } from "./MatrixCell";
import type { EndpointDescriptor } from "../types/session";

export interface KeyboardNavCell {
  desc: EndpointDescriptor;
}

export interface UseKeyboardNavResult {
  focusedCell: () => { row: number; col: number } | null;
  setFocusedCell: (cell: { row: number; col: number } | null) => void;
  registerCellActions: (row: number, col: number, actions: MatrixCellActions) => void;
  handleGridKeyDown: (e: KeyboardEvent) => void;
}

export function useKeyboardNav(
  getChannels: () => KeyboardNavCell[],
  getMixes: () => KeyboardNavCell[],
  isEqOpen: () => boolean,
  openCellEq: (source: EndpointDescriptor, sink: EndpointDescriptor) => void,
): UseKeyboardNavResult {
  const [focusedCell, setFocusedCell] = createSignal<{ row: number; col: number } | null>(null);
  const cellActionsMap = new Map<string, MatrixCellActions>();

  function cellKey(row: number, col: number): string {
    return `${row},${col}`;
  }

  function registerCellActions(row: number, col: number, actions: MatrixCellActions) {
    cellActionsMap.set(cellKey(row, col), actions);
  }

  function getFocusedActions(): MatrixCellActions | undefined {
    const fc = focusedCell();
    if (!fc) return undefined;
    return cellActionsMap.get(cellKey(fc.row, fc.col));
  }

  function handleGridKeyDown(e: KeyboardEvent) {
    if (isEqOpen()) return;

    const numRows = getChannels().length;
    const numCols = getMixes().length;
    if (numRows === 0 || numCols === 0) return;

    const fc = focusedCell();

    if (e.key === "Tab" && !fc) {
      e.preventDefault();
      setFocusedCell({ row: 0, col: 0 });
      return;
    }

    if (!fc) return;

    switch (e.key) {
      case "ArrowRight": {
        e.preventDefault();
        const nextCol = fc.col < numCols - 1 ? fc.col + 1 : 0;
        setFocusedCell({ row: fc.row, col: nextCol });
        break;
      }
      case "ArrowLeft": {
        e.preventDefault();
        const prevCol = fc.col > 0 ? fc.col - 1 : numCols - 1;
        setFocusedCell({ row: fc.row, col: prevCol });
        break;
      }
      case "ArrowDown": {
        e.preventDefault();
        if (e.shiftKey) {
          getFocusedActions()?.adjustVolume(-0.05);
        } else if (e.altKey) {
          getFocusedActions()?.adjustVolume(-0.01);
        } else {
          const nextRow = fc.row < numRows - 1 ? fc.row + 1 : 0;
          setFocusedCell({ row: nextRow, col: fc.col });
        }
        break;
      }
      case "ArrowUp": {
        e.preventDefault();
        if (e.shiftKey) {
          getFocusedActions()?.adjustVolume(0.05);
        } else if (e.altKey) {
          getFocusedActions()?.adjustVolume(0.01);
        } else {
          const prevRow = fc.row > 0 ? fc.row - 1 : numRows - 1;
          setFocusedCell({ row: prevRow, col: fc.col });
        }
        break;
      }
      case "Escape": {
        e.preventDefault();
        setFocusedCell(null);
        break;
      }
      case "m":
      case "M": {
        e.preventDefault();
        getFocusedActions()?.toggleMute();
        break;
      }
      case "s":
      case "S": {
        e.preventDefault();
        const chS = getChannels()[fc.row];
        const mixS = getMixes()[fc.col];
        if (chS && mixS) {
          openCellEq(chS.desc, mixS.desc);
        }
        break;
      }
      case "e":
      case "E": {
        e.preventDefault();
        const ch = getChannels()[fc.row];
        const mix = getMixes()[fc.col];
        if (ch && mix) {
          openCellEq(ch.desc, mix.desc);
        }
        break;
      }
      case "+":
      case "=": {
        e.preventDefault();
        getFocusedActions()?.adjustVolume(0.01);
        break;
      }
      case "-":
      case "_": {
        e.preventDefault();
        getFocusedActions()?.adjustVolume(-0.01);
        break;
      }
      default:
        return;
    }
  }

  return { focusedCell, setFocusedCell, registerCellActions, handleGridKeyDown };
}
