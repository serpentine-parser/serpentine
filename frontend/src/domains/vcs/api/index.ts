import { VcsRef } from '../model/types';
import { VcsRefsResponse } from './types';

const API_BASE = (import.meta.env.VITE_API_URL ?? '').replace(/\/$/, '');

export async function fetchVcsRefs(): Promise<{ refs: VcsRef[]; available: boolean }> {
  try {
    const response = await fetch(`${API_BASE}/api/vcs/refs`);
    if (!response.ok) return { refs: [], available: false };
    const data: VcsRefsResponse = await response.json();
    return { refs: data.refs, available: data.available };
  } catch {
    return { refs: [], available: false };
  }
}
