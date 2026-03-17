import type {
  DisplaySettings,
  LayoutSettings,
  NodePersistState,
  PersistedLayoutData,
} from "../model/layoutTypes";
import { DEFAULT_LAYOUT_SETTINGS } from "../model/layoutTypes";

const CURRENT_VERSION = 1 as const;
const INFINITY_SENTINEL = "__Infinity__";

function serializeSettings(
  settings: Partial<DisplaySettings>,
): Record<string, unknown> {
  const out: Record<string, unknown> = { ...settings };
  if (settings.visibleEdgeDepth === Infinity) {
    out.visibleEdgeDepth = INFINITY_SENTINEL;
  }
  return out;
}

function deserializeSettings(
  raw: Record<string, unknown>,
): Partial<DisplaySettings> {
  const out: Partial<DisplaySettings> = {};
  if (typeof raw.includeThirdPartyPackages === "boolean") {
    out.includeThirdPartyPackages = raw.includeThirdPartyPackages;
  }
  if (typeof raw.includeStandardPackages === "boolean") {
    out.includeStandardPackages = raw.includeStandardPackages;
  }
  if (raw.visibleEdgeDepth === INFINITY_SENTINEL) {
    out.visibleEdgeDepth = Infinity;
  } else if (typeof raw.visibleEdgeDepth === "number") {
    out.visibleEdgeDepth = raw.visibleEdgeDepth;
  }
  if (typeof raw.selectorQuery === "string") out.selectorQuery = raw.selectorQuery;
  if (typeof raw.selectorExclude === "string") out.selectorExclude = raw.selectorExclude;
  return out;
}

function makeEmptyData(): PersistedLayoutData {
  return { version: CURRENT_VERSION, settings: {}, nodes: {}, layoutSettings: {} };
}

class LayoutCache {
  private readonly key: string;
  private data: PersistedLayoutData;
  private saveTimer: ReturnType<typeof setTimeout> | null = null;

  constructor() {
    this.key = `serpentine:${window.location.origin}`;
    this.data = this.load();
  }

  // --- persistence ---

  private load(): PersistedLayoutData {
    try {
      const raw = localStorage.getItem(this.key);
      if (!raw) return makeEmptyData();
      const parsed = JSON.parse(raw);
      if (parsed?.version !== CURRENT_VERSION) return makeEmptyData();
      return {
        version: CURRENT_VERSION,
        settings: deserializeSettings(parsed.settings ?? {}),
        nodes: parsed.nodes ?? {},
        layoutSettings: parsed.layoutSettings ?? {},
      };
    } catch {
      return makeEmptyData();
    }
  }

  private scheduleSave(): void {
    if (this.saveTimer !== null) clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null;
      try {
        const serializable = {
          version: this.data.version,
          settings: serializeSettings(this.data.settings),
          nodes: this.data.nodes,
          layoutSettings: this.data.layoutSettings,
        };
        localStorage.setItem(this.key, JSON.stringify(serializable));
      } catch {
        // quota exceeded or private browsing — silently ignore
      }
    }, 200);
  }

  // --- display settings ---

  getDisplaySettings(): Partial<DisplaySettings> {
    return { ...this.data.settings };
  }

  saveDisplaySettings(settings: Partial<DisplaySettings>): void {
    this.data.settings = { ...this.data.settings, ...settings };
    this.scheduleSave();
  }

  // --- layout settings ---

  getLayoutSettings(): LayoutSettings {
    return { ...DEFAULT_LAYOUT_SETTINGS, ...this.data.layoutSettings };
  }

  saveLayoutSettings(settings: Partial<LayoutSettings>): void {
    this.data.layoutSettings = { ...this.data.layoutSettings, ...settings };
    this.scheduleSave();
  }

  // --- per-node state ---

  getNodeState(id: string): NodePersistState | undefined {
    return this.data.nodes[id];
  }

  setNodeCollapsed(id: string, collapsed: boolean): void {
    this.data.nodes[id] = { ...this.data.nodes[id], collapsed };
    this.scheduleSave();
  }

  setPinnedPosition(id: string, x: number, y: number): void {
    this.data.nodes[id] = {
      ...this.data.nodes[id],
      pinned: true,
      pinnedX: x,
      pinnedY: y,
    };
    this.scheduleSave();
  }

  clearPinnedPosition(id: string): void {
    if (!this.data.nodes[id]) return;
    const { pinned: _p, pinnedX: _x, pinnedY: _y, ...rest } = this.data.nodes[id];
    if (Object.keys(rest).length === 0) {
      delete this.data.nodes[id];
    } else {
      this.data.nodes[id] = rest;
    }
    this.scheduleSave();
  }

  getAllNodeStates(): Record<string, NodePersistState> {
    return { ...this.data.nodes };
  }

  // --- bulk queries ---

  getPinnedPositions(): Record<string, { x: number; y: number }> {
    const out: Record<string, { x: number; y: number }> = {};
    for (const [id, state] of Object.entries(this.data.nodes)) {
      if (state.pinned && state.pinnedX !== undefined && state.pinnedY !== undefined) {
        out[id] = { x: state.pinnedX, y: state.pinnedY };
      }
    }
    return out;
  }

  // --- reset ---

  clear(): void {
    this.data = makeEmptyData();
    try { localStorage.removeItem(this.key); } catch { /* ignore */ }
  }

  // --- pruning ---

  pruneStaleNodes(liveNodeIds: Set<string>): void {
    let pruned = false;
    for (const id of Object.keys(this.data.nodes)) {
      if (!liveNodeIds.has(id)) {
        delete this.data.nodes[id];
        pruned = true;
      }
    }
    if (pruned) this.scheduleSave();
  }
}

export const layoutCache = new LayoutCache();
