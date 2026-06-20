import type { ComparisonState } from '../model/types';

const KEY = 'serpentine.vcs.comparison';

export function saveComparison(comparison: ComparisonState): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(comparison));
  } catch {
    // quota exceeded or private browsing — ignore
  }
}

export function loadComparison(): ComparisonState | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (typeof parsed?.from === 'string' && typeof parsed?.to === 'string') {
      return { from: parsed.from, to: parsed.to };
    }
  } catch {
    // malformed — ignore
  }
  return null;
}
