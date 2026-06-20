import { useRef, useState, useEffect, useCallback } from 'react';
import { IconGitCompare, IconChevronDown, IconFlag, IconLoader2 } from '@tabler/icons-react';
import { useGraphStore } from '@store';
import { bus } from '../../../app/bus';
import { sendWsMessage } from '../../../app/actors';
import type { VcsRef, RefSelection, ComparisonState } from '@domains/vcs';

// ─── helpers ────────────────────────────────────────────────────────────────

function isActive(c: ComparisonState) {
  return c.from !== '@current' || c.to !== '@current';
}

function labelFor(ref: RefSelection, refs: VcsRef[]): string {
  if (ref === '@current') return 'Current (live)';
  if (ref === '@start') return 'At checkpoint';
  return refs.find((r) => r.id === ref)?.display ?? ref;
}

// ─── RefDropdown ─────────────────────────────────────────────────────────────

interface RefDropdownProps {
  label: string;
  value: RefSelection;
  refs: VcsRef[];
  hideStart?: boolean;
  hideCurrent?: boolean;
  onChange: (ref: RefSelection) => void;
}

function RefDropdown({ label, value, refs, hideStart, hideCurrent, onChange }: RefDropdownProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [activeIndex, setActiveIndex] = useState(-1);
  const containerRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) { setSearch(''); setActiveIndex(-1); return; }
    searchRef.current?.focus();
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  const q = search.toLowerCase();
  const match = (r: VcsRef) => !q || r.display.toLowerCase().includes(q);

  const branches = refs.filter((r) => r.kind === 'branch' && match(r));
  const tags = refs.filter((r) => r.kind === 'tag' && match(r));
  const commits = refs.filter((r) => r.kind === 'commit' && match(r));

  // Flat ordered list of selectable values for keyboard navigation
  const items: RefSelection[] = [
    ...(!hideCurrent && (!q || 'current live'.includes(q)) ? ['@current' as RefSelection] : []),
    ...(!hideStart && (!q || 'checkpoint'.includes(q)) ? ['@start' as RefSelection] : []),
    ...branches.map((r) => r.id),
    ...tags.map((r) => r.id),
    ...commits.map((r) => r.id),
  ];

  const select = (v: RefSelection) => { onChange(v); setOpen(false); };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, items.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, -1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (activeIndex >= 0 && items[activeIndex]) select(items[activeIndex]);
    } else if (e.key === 'Escape') {
      setOpen(false);
    }
  };

  // Scroll active item into view
  useEffect(() => {
    if (activeIndex < 0 || !listRef.current) return;
    const el = listRef.current.querySelectorAll<HTMLElement>('[data-item]')[activeIndex];
    el?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex]);

  return (
    <div ref={containerRef} className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1 px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 transition-colors"
        title={label}
      >
        <span className="max-w-[120px] truncate">{labelFor(value, refs)}</span>
        <IconChevronDown size={12} />
      </button>

      {open && (
        <div className="absolute top-full mt-1 left-0 z-50 min-w-[220px] rounded-md border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg text-xs">
          <div className="p-1.5 border-b border-slate-100 dark:border-slate-700">
            <input
              ref={searchRef}
              type="text"
              value={search}
              onChange={(e) => { setSearch(e.target.value); setActiveIndex(-1); }}
              onKeyDown={handleKeyDown}
              placeholder="Search…"
              className="w-full px-2 py-1 text-xs rounded border border-slate-200 dark:border-slate-600 bg-slate-50 dark:bg-slate-900 text-slate-800 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>
          <div ref={listRef} className="max-h-64 overflow-y-auto">
            {!hideCurrent && (!q || 'current live'.includes(q)) && (
              <DropdownItem data-item active={value === '@current'} highlighted={items.indexOf('@current') === activeIndex} onClick={() => select('@current')} onMouseEnter={() => setActiveIndex(items.indexOf('@current'))}>
                Current (live)
              </DropdownItem>
            )}
            {!hideStart && (!q || 'checkpoint'.includes(q)) && (
              <DropdownItem data-item active={value === '@start'} highlighted={items.indexOf('@start') === activeIndex} onClick={() => select('@start')} onMouseEnter={() => setActiveIndex(items.indexOf('@start'))}>
                At checkpoint
              </DropdownItem>
            )}

            {branches.length > 0 && (
              <>
                <SectionHeader>Branches</SectionHeader>
                {branches.map((r) => (
                  <DropdownItem key={r.id} data-item active={value === r.id} highlighted={items.indexOf(r.id) === activeIndex} onClick={() => select(r.id)} onMouseEnter={() => setActiveIndex(items.indexOf(r.id))}>
                    {r.display}
                  </DropdownItem>
                ))}
              </>
            )}

            {tags.length > 0 && (
              <>
                <SectionHeader>Tags</SectionHeader>
                {tags.map((r) => (
                  <DropdownItem key={r.id} data-item active={value === r.id} highlighted={items.indexOf(r.id) === activeIndex} onClick={() => select(r.id)} onMouseEnter={() => setActiveIndex(items.indexOf(r.id))}>
                    {r.display}
                  </DropdownItem>
                ))}
              </>
            )}

            {commits.length > 0 && (
              <>
                <SectionHeader>Recent commits</SectionHeader>
                {commits.map((r) => (
                  <DropdownItem key={r.id} data-item active={value === r.id} highlighted={items.indexOf(r.id) === activeIndex} onClick={() => select(r.id)} onMouseEnter={() => setActiveIndex(items.indexOf(r.id))}>
                    <span className="font-mono">{r.display}</span>
                  </DropdownItem>
                ))}
              </>
            )}

            {branches.length === 0 && tags.length === 0 && commits.length === 0 && !(!q || 'current live'.includes(q)) && !(!q || 'checkpoint'.includes(q)) && (
              <div className="px-3 py-2 text-slate-400 dark:text-slate-500">No matches</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500 border-t border-slate-100 dark:border-slate-700 mt-1 first:mt-0 first:border-t-0">
      {children}
    </div>
  );
}

function DropdownItem({ active, highlighted, onClick, onMouseEnter, children, ...rest }: { active: boolean; highlighted?: boolean; onClick: () => void; onMouseEnter?: () => void; children: React.ReactNode; [key: string]: unknown }) {
  return (
    <button
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      {...rest}
      className={`w-full text-left px-3 py-1.5 transition-colors ${
        highlighted ? 'bg-slate-100 dark:bg-slate-700' : 'hover:bg-slate-100 dark:hover:bg-slate-700'
      } ${active ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-slate-700 dark:text-slate-200'}`}
    >
      {children}
    </button>
  );
}

// ─── VcsRefPicker ─────────────────────────────────────────────────────────────

export function VcsRefPicker() {
  const vcsAvailable = useGraphStore((s) => s.vcsAvailable);
  const vcsRefs = useGraphStore((s) => s.vcsRefs);
  const comparison = useGraphStore((s) => s.comparison);
  const setComparison = useGraphStore((s) => s.setComparison);
  const clearComparison = useGraphStore((s) => s.clearComparison);
  const [loading, setLoading] = useState(false);
  const loadingRef = useRef(false);

  useEffect(() => {
    return bus.subscribe((event) => {
      if (event.type === 'GRAPH_UPDATED' && loadingRef.current) {
        loadingRef.current = false;
        setLoading(false);
      }
    });
  }, []);

  const startLoading = useCallback(() => {
    loadingRef.current = true;
    setLoading(true);
  }, []);

  const active = isActive(comparison);

  const handleFromChange = (from: RefSelection) => {
    const next: ComparisonState = { from, to: comparison.to };
    if (!isActive(next)) {
      clearComparison();
      bus.publish({ type: 'VCS_COMPARISON_CLEARED' });
    } else {
      setComparison(next);
      startLoading();
      bus.publish({ type: 'VCS_COMPARISON_SET', from: next.from, to: next.to });
    }
  };

  const handleToChange = (to: RefSelection) => {
    const next: ComparisonState = { from: comparison.from, to };
    if (!isActive(next)) {
      clearComparison();
      bus.publish({ type: 'VCS_COMPARISON_CLEARED' });
    } else {
      setComparison(next);
      startLoading();
      bus.publish({ type: 'VCS_COMPARISON_SET', from: next.from, to: next.to });
    }
  };

  const handleMarkCheckpoint = () => {
    sendWsMessage({ action: 'update_start' });
    const next: ComparisonState = { from: '@start', to: comparison.to };
    setComparison(next);
    if (isActive(next)) {
      startLoading();
      bus.publish({ type: 'VCS_COMPARISON_SET', from: next.from, to: next.to });
    }
  };

  if (!vcsAvailable) {
    return (
      <div className="flex items-center gap-2">
        <div className="flex items-center gap-1.5 text-xs text-slate-500 dark:text-slate-400">
          <IconGitCompare size={14} strokeWidth={1.75} />
          <span className="hidden sm:inline">compare</span>
        </div>
        <span className="px-2 py-1 text-xs text-slate-500 dark:text-slate-400">At checkpoint</span>
        <span className="text-slate-400 dark:text-slate-500 text-xs">←</span>
        <span className="px-2 py-1 text-xs text-slate-500 dark:text-slate-400">Current (live)</span>
        <button
          onClick={handleMarkCheckpoint}
          title="Mark current state as checkpoint — clears the change overlay"
          className="flex items-center gap-1 px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 transition-colors"
        >
          <IconFlag size={12} />
          <span className="hidden sm:inline">checkpoint</span>
        </button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <div className="flex items-center gap-1.5 text-xs text-slate-500 dark:text-slate-400">
        {loading
          ? <IconLoader2 size={14} strokeWidth={1.75} className="animate-spin" />
          : <IconGitCompare size={14} strokeWidth={1.75} />
        }
        <span className="hidden sm:inline">compare</span>
      </div>

      <RefDropdown
        label="from"
        value={comparison.from}
        refs={vcsRefs}
        hideCurrent={false}
        onChange={handleFromChange}
      />

      <span className="text-slate-400 dark:text-slate-500 text-xs">←</span>

      <RefDropdown
        label="to"
        value={comparison.to}
        refs={vcsRefs}
        hideStart={false}
        onChange={handleToChange}
      />

      {active && (
        <button
          onClick={handleMarkCheckpoint}
          title="Mark current state as checkpoint — clears the change overlay"
          className="flex items-center gap-1 px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 transition-colors"
        >
          <IconFlag size={12} />
          <span className="hidden sm:inline">checkpoint</span>
        </button>
      )}
    </div>
  );
}
