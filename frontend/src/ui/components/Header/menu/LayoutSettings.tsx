import { DEFAULT_LAYOUT_SETTINGS, LayoutSettings } from "@domains/graph/model/layoutTypes";
import { IconArrowsMaximize, IconArrowsMinimize } from "@tabler/icons-react";

type NumberInputRowProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
};

function NumberInputRow({ label, value, min, max, step, onChange }: NumberInputRowProps) {
  const clamp = (v: number) => Math.min(max, Math.max(min, v));

  return (
    <div className="flex items-center gap-2">
      <span className="flex-1 text-xs text-gray-600 dark:text-gray-400">{label}</span>
      <div className="flex items-center border border-gray-200 dark:border-slate-600 rounded overflow-hidden">
        <button
          onClick={() => onChange(clamp(+(value - step).toFixed(10)))}
          disabled={value <= min}
          className="px-1.5 py-0.5 text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-slate-700 disabled:opacity-30 disabled:cursor-not-allowed"
        >
          −
        </button>
        <input
          type="number"
          min={min}
          max={max}
          step={step}
          value={value}
          onChange={(e) => {
            const v = parseFloat(e.target.value);
            if (!isNaN(v)) onChange(clamp(v));
          }}
          className="w-12 text-center text-xs py-0.5 bg-transparent text-gray-900 dark:text-gray-100 focus:outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
        />
        <button
          onClick={() => onChange(clamp(+(value + step).toFixed(10)))}
          disabled={value >= max}
          className="px-1.5 py-0.5 text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-slate-700 disabled:opacity-30 disabled:cursor-not-allowed"
        >
          +
        </button>
      </div>
    </div>
  );
}

export interface LayoutSettingsPanelProps {
  layoutSettings: LayoutSettings;
  setLayoutSettings: (patch: Partial<LayoutSettings>) => void;
  flipLayoutDirection: () => void;
  expandAll: () => void;
  collapseAll: () => void;
}

export function LayoutSettingsPanel({
  layoutSettings,
  setLayoutSettings,
  flipLayoutDirection,
  expandAll,
  collapseAll,
}: LayoutSettingsPanelProps) {
  const patch = (key: keyof LayoutSettings, value: LayoutSettings[keyof LayoutSettings]) =>
    setLayoutSettings({ [key]: value } as Partial<LayoutSettings>);

  return (
    <div className="border-t border-gray-100 dark:border-slate-700 mt-1 pt-2 space-y-2">
      {/* Expand / Collapse all */}
      <div className="px-2">
        <div className="flex rounded-md bg-gray-100 dark:bg-slate-700 p-0.5 text-xs gap-0.5">
          <button
            onClick={expandAll}
            className="flex-1 flex items-center justify-center gap-1 py-1 rounded text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-white dark:hover:bg-slate-600 transition-colors"
            title="Expand all nodes"
          >
            <IconArrowsMaximize size={12} />
            Expand all
          </button>
          <button
            onClick={collapseAll}
            className="flex-1 flex items-center justify-center gap-1 py-1 rounded text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-white dark:hover:bg-slate-600 transition-colors"
            title="Collapse all nodes"
          >
            <IconArrowsMinimize size={12} />
            Collapse all
          </button>
        </div>
      </div>

      <div className="px-2 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
        Layout
      </div>

      {/* Direction toggle */}
      <div className="px-2">
        <div className="flex rounded-md bg-gray-100 dark:bg-slate-700 p-0.5 text-xs">
          <button
            onClick={() => layoutSettings.rootDirection !== "RIGHT" && flipLayoutDirection()}
            className={`flex-1 py-1 rounded transition-colors ${
              layoutSettings.rootDirection === "RIGHT"
                ? "bg-white dark:bg-slate-600 text-gray-900 dark:text-gray-100 shadow-sm"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200"
            }`}
          >
            Horizontal
          </button>
          <button
            onClick={() => layoutSettings.rootDirection !== "DOWN" && flipLayoutDirection()}
            className={`flex-1 py-1 rounded transition-colors ${
              layoutSettings.rootDirection === "DOWN"
                ? "bg-white dark:bg-slate-600 text-gray-900 dark:text-gray-100 shadow-sm"
                : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200"
            }`}
          >
            Vertical
          </button>
        </div>
      </div>

      {/* ELK spacing inputs */}
      <div className="px-2 space-y-1.5">
        <NumberInputRow label="Root layer gap" value={layoutSettings.rootNodeBetweenLayers} min={20} max={400} step={10} onChange={(v) => patch("rootNodeBetweenLayers", v)} />
        <NumberInputRow label="Root node gap" value={layoutSettings.rootNodeNode} min={20} max={300} step={10} onChange={(v) => patch("rootNodeNode", v)} />
        <NumberInputRow label="Child layer gap" value={layoutSettings.childNodeBetweenLayers} min={10} max={200} step={5} onChange={(v) => patch("childNodeBetweenLayers", v)} />
        <NumberInputRow label="Child node gap" value={layoutSettings.childNodeNode} min={10} max={200} step={5} onChange={(v) => patch("childNodeNode", v)} />
        <NumberInputRow label="Edge-node gap" value={layoutSettings.edgeNode} min={5} max={150} step={5} onChange={(v) => patch("edgeNode", v)} />
        <NumberInputRow label="Component gap" value={layoutSettings.componentComponent} min={5} max={100} step={5} onChange={(v) => patch("componentComponent", v)} />
        <NumberInputRow label="Padding" value={layoutSettings.padding} min={5} max={150} step={5} onChange={(v) => patch("padding", v)} />
      </div>

      <div className="px-2 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
        Edges
      </div>
      <div className="px-2 space-y-1.5">
        <NumberInputRow label="Curvature" value={layoutSettings.edgeCurvature} min={0} max={150} step={5} onChange={(v) => patch("edgeCurvature", v)} />
        <NumberInputRow label="Stroke width" value={layoutSettings.edgeStrokeWidth} min={0.5} max={5} step={0.1} onChange={(v) => patch("edgeStrokeWidth", v)} />
      </div>

      {/* Reset */}
      <div className="px-2 pb-1">
        <button
          onClick={() => setLayoutSettings({ ...DEFAULT_LAYOUT_SETTINGS })}
          className="w-full text-xs text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 py-1 border border-gray-200 dark:border-slate-600 rounded hover:bg-gray-50 dark:hover:bg-slate-700 transition-colors"
        >
          Reset to defaults
        </button>
      </div>
    </div>
  );
}
