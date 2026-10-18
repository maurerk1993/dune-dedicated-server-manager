import { useRef, useState } from "react";
import type { DragEvent, ReactNode } from "react";
import { DragHandleDots2Icon } from "@radix-ui/react-icons";
import { createPortal } from "react-dom";
import {
  DEFAULT_DASHBOARD_ORDER, DASHBOARD_SECTION_LABELS, moveDashboardSection,
  readDashboardOrder, saveDashboardOrder,
} from "../../utils/dashboard-layout";
import type { DashboardSectionId } from "../../utils/dashboard-layout";

type DropTarget = { id: DashboardSectionId; edge: "before" | "after" };

/** Presentation-only ordering: no server calls or changes to child action guards. */
export default function DashboardLayout({ sections, attention, resetTarget }: {
  sections: Record<DashboardSectionId, (handle: ReactNode) => ReactNode>;
  attention?: ReactNode;
  resetTarget?: HTMLElement | null;
}) {
  const [order, setOrder] = useState<DashboardSectionId[]>(() => {
    try { return readDashboardOrder(window.localStorage); }
    catch { return [...DEFAULT_DASHBOARD_ORDER]; }
  });
  const dragging = useRef<DashboardSectionId | null>(null);
  const [draggedId, setDraggedId] = useState<DashboardSectionId | null>(null);
  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const [saveFailed, setSaveFailed] = useState(false);
  const handles = useRef<Partial<Record<DashboardSectionId, HTMLButtonElement | null>>>({});

  function clearDrag() {
    dragging.current = null;
    setDraggedId(null);
    setDropTarget(null);
  }

  function commit(next: DashboardSectionId[], message: string, focus?: DashboardSectionId) {
    setOrder(next);
    try { setSaveFailed(!saveDashboardOrder(window.localStorage, next)); }
    catch { setSaveFailed(true); }
    setAnnouncement(message);
    if (focus) requestAnimationFrame(() => handles.current[focus]?.focus());
  }

  function move(id: DashboardSectionId, target: DashboardSectionId, edge: DropTarget["edge"], restoreFocus = true) {
    const next = moveDashboardSection(order, id, target, edge);
    commit(next, `${DASHBOARD_SECTION_LABELS[id]} moved to position ${next.indexOf(id) + 1} of ${next.length}.`, restoreFocus ? id : undefined);
  }

  function targetAt(event: DragEvent<HTMLDivElement>, id: DashboardSectionId): DropTarget {
    const bounds = event.currentTarget.getBoundingClientRect();
    return { id, edge: event.clientY < bounds.top + bounds.height / 2 ? "before" : "after" };
  }

  return (
    <div className="dashboard-layout"
      onDragOverCapture={(event) => {
        // This surface accepts section handles, not external files or URLs.
        if (!dragging.current) { event.preventDefault(); event.dataTransfer.dropEffect = "none"; }
      }}
      onDropCapture={(event) => {
        if (!dragging.current) { event.preventDefault(); event.stopPropagation(); }
      }}>
      {resetTarget ? createPortal(
        <button type="button" className="dashboard-reset-layout" onClick={() => {
          clearDrag();
          commit([...DEFAULT_DASHBOARD_ORDER], "Default dashboard order restored.");
        }}>Reset layout</button>, resetTarget) : null}
      <div className="dashboard-layout-announcement" role="status" aria-live="polite">{announcement}</div>
      {saveFailed ? <div role="status" className="dashboard-section-note">
        Layout changed for this session, but could not be saved on this computer.
      </div> : null}
      {attention}
      {order.map((id, index) => (
        <div key={id} className="dashboard-layout-section" data-dashboard-section={id}
          data-dragging={draggedId === id || undefined}
          data-drop-edge={dropTarget?.id === id ? dropTarget.edge : undefined}
          onDragOver={(event) => {
            if (!dragging.current || dragging.current === id) return;
            event.preventDefault();
            event.dataTransfer.dropEffect = "move";
            const target = targetAt(event, id);
            setDropTarget((current) => current?.id === target.id && current.edge === target.edge ? current : target);
          }}
          onDragLeave={(event) => {
            if (!(event.relatedTarget instanceof Node) || !event.currentTarget.contains(event.relatedTarget)) {
              setDropTarget((current) => current?.id === id ? null : current);
            }
          }}
          onDrop={(event) => {
            const source = dragging.current;
            if (!source) return;
            event.preventDefault();
            event.stopPropagation();
            if (source !== id) move(source, id, targetAt(event, id).edge, false);
            clearDrag();
          }}>
          {sections[id](
            <button type="button" className="dashboard-drag-handle" draggable
              ref={(element) => { handles.current[id] = element; }}
              aria-label={`Move ${DASHBOARD_SECTION_LABELS[id]}. Drag or use Arrow Up and Arrow Down.`}
              title={`Drag ${DASHBOARD_SECTION_LABELS[id]} to reorder, or use arrow keys`}
              onDragStart={(event) => {
                dragging.current = id;
                setDraggedId(id);
                event.dataTransfer.effectAllowed = "move";
                event.dataTransfer.setData("text/plain", id);
                const section = event.currentTarget.closest("[data-dashboard-section]");
                if (section) event.dataTransfer.setDragImage(section, 24, 16);
              }}
              onDragEnd={clearDrag}
              onKeyDown={(event) => {
                const offset = event.key === "ArrowUp" ? -1 : event.key === "ArrowDown" ? 1 : 0;
                if (!offset) return;
                event.preventDefault();
                const target = order[index + offset];
                if (target) move(id, target, offset < 0 ? "before" : "after");
              }}>
              <DragHandleDots2Icon aria-hidden />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
