import { transformData, TransformedData } from './mappers';

export { transformData, transformFlowGraph } from './mappers';
export type { TransformedData } from './mappers';

export async function loadData(selectQuery?: string, excludeQuery?: string, stateQuery?: string): Promise<TransformedData> {
  try {
    const API_BASE = (import.meta.env.VITE_API_URL || "").replace(/\/$/, "");
    let endpoint = API_BASE ? `${API_BASE}/api/graph` : "/api/graph";

    const params = new URLSearchParams();
    if (selectQuery?.trim()) params.append("select", selectQuery);
    if (excludeQuery?.trim()) params.append("exclude", excludeQuery);
    if (stateQuery?.trim()) params.append("state", stateQuery);
    const queryString = params.toString();
    if (queryString) endpoint += `?${queryString}`;

    const response = await fetch(endpoint);
    if (!response.ok) throw new Error(`Failed to load data: ${response.statusText}`);
    const rawData = await response.json();
    return transformData(rawData);
  } catch (error) {
    console.error("Error loading data:", error);
    return { nodes: [], edges: [] };
  }
}
