import { Edge, Node } from "@domains/graph/model/types";
import { IconArrowsMaximize } from "@tabler/icons-react";
import Prism from "prismjs";
import "prismjs/components/prism-javascript";
import "prismjs/components/prism-jsx";
import "prismjs/components/prism-python";
import "prismjs/components/prism-tsx";
import "prismjs/components/prism-typescript";
import "prismjs/themes/prism-okaidia.css";
import { useEffect, useRef, useState } from "react";

interface DetailPanelProps {
  node?: Node;
  cfgNode?: Node;
  onClose: () => void;
  allEdges: Edge[];
  visibleEdges: Edge[];
  findNodeById: (id: string) => Node | undefined;
  selectNode: (id: string | null) => void;
  expandParentNodes: (id: string) => void;
}

function getPrismLanguage(node: Node): string {
  const ext = node.file_path?.split('.').pop()?.toLowerCase();
  if (ext === 'tsx') return 'tsx';
  if (ext === 'ts') return 'typescript';
  if (ext === 'jsx') return 'jsx';
  if (ext === 'js') return 'javascript';
  return 'python';
}

function applyModalSearchMarks(el: HTMLElement, query: string): number {
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];
  let n = walker.nextNode();
  while (n) {
    textNodes.push(n as Text);
    n = walker.nextNode();
  }
  const lowerQuery = query.toLowerCase();
  let count = 0;
  for (const textNode of textNodes) {
    const text = textNode.textContent ?? '';
    const lowerText = text.toLowerCase();
    if (!lowerText.includes(lowerQuery)) continue;
    const frag = document.createDocumentFragment();
    let last = 0;
    let idx = lowerText.indexOf(lowerQuery, last);
    while (idx !== -1) {
      if (idx > last) frag.appendChild(document.createTextNode(text.slice(last, idx)));
      const mark = document.createElement('mark');
      mark.dataset.match = String(count);
      mark.textContent = text.slice(idx, idx + query.length);
      mark.style.borderRadius = '2px';
      mark.style.color = '#1a1a1a';
      frag.appendChild(mark);
      count++;
      last = idx + query.length;
      idx = lowerText.indexOf(lowerQuery, last);
    }
    if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
    textNode.parentNode!.replaceChild(frag, textNode);
  }
  return count;
}

function highlightMatchAtIndex(marks: Element[], index: number) {
  marks.forEach((m, i) => {
    (m as HTMLElement).style.backgroundColor = i === index ? '#f59e0b' : '#fef9c3';
  });
}

export function DetailPanel({ node, cfgNode, onClose, allEdges, visibleEdges, findNodeById, selectNode, expandParentNodes }: DetailPanelProps) {
  const codeRef = useRef<HTMLElement>(null);

  const [showCodeModal, setShowCodeModal] = useState(false);
  const modalCodeRef = useRef<HTMLElement | null>(null);

  const [panelWidth, setPanelWidth] = useState(360);
  const dragState = useRef<{ startX: number; startWidth: number } | null>(null);

  const [modalSearchQuery, setModalSearchQuery] = useState('');
  const [matchCount, setMatchCount] = useState(0);
  const [currentMatch, setCurrentMatch] = useState(0);
  const modalSearchRef = useRef<HTMLInputElement>(null);
  const highlightedHtml = useRef<string>('');
  const matchMarksRef = useRef<Element[]>([]);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragState.current) return;
      const delta = e.clientX - dragState.current.startX;
      setPanelWidth(Math.max(240, Math.min(600, dragState.current.startWidth + delta)));
    };
    const onUp = () => {
      dragState.current = null;
      document.body.style.cursor = "";
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    return () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
  }, []);

  // Highlight code when component mounts or code changes
  useEffect(() => {
    if (!node || !codeRef.current || !node.code_block) return;
    codeRef.current.textContent = node.code_block;
    codeRef.current.classList.remove("highlighted");
    Prism.highlightElement(codeRef.current);
  }, [node?.id, node?.code_block]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'f' && showCodeModal) {
        e.preventDefault();
        modalSearchRef.current?.focus();
        return;
      }
      if (e.key === 'Escape') {
        if (showCodeModal && modalSearchQuery) {
          setModalSearchQuery('');
        } else {
          setShowCodeModal(false);
        }
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [showCodeModal, modalSearchQuery]);

  useEffect(() => {
    if (!node) return;
    if (showCodeModal && modalCodeRef.current && node.code_block) {
      modalCodeRef.current.textContent = node.code_block;
      Prism.highlightElement(modalCodeRef.current);
      highlightedHtml.current = modalCodeRef.current.innerHTML;
      setModalSearchQuery('');
      setMatchCount(0);
      setCurrentMatch(0);
      matchMarksRef.current = [];
    }
  }, [showCodeModal, node?.code_block]);

  useEffect(() => {
    if (!showCodeModal || !modalCodeRef.current) return;
    modalCodeRef.current.innerHTML = highlightedHtml.current;
    matchMarksRef.current = [];
    if (!modalSearchQuery.trim()) {
      setMatchCount(0);
      setCurrentMatch(0);
      return;
    }
    const count = applyModalSearchMarks(modalCodeRef.current, modalSearchQuery);
    matchMarksRef.current = Array.from(modalCodeRef.current.querySelectorAll('[data-match]'));
    setMatchCount(count);
    setCurrentMatch(count > 0 ? 1 : 0);
    if (count > 0) {
      highlightMatchAtIndex(matchMarksRef.current, 0);
      (matchMarksRef.current[0] as HTMLElement).scrollIntoView({ block: 'nearest' });
    }
  }, [modalSearchQuery, showCodeModal]);

  useEffect(() => {
    if (matchMarksRef.current.length === 0 || currentMatch === 0) return;
    highlightMatchAtIndex(matchMarksRef.current, currentMatch - 1);
    (matchMarksRef.current[currentMatch - 1] as HTMLElement)?.scrollIntoView({ block: 'nearest' });
  }, [currentMatch]);

  if (cfgNode) {
    return (
      <CfgDetailContent
        node={cfgNode}
        onClose={onClose}
        onSelectNode={selectNode}
        panelWidth={panelWidth}
        onResizeStart={(e: React.MouseEvent) => {
          e.preventDefault();
          dragState.current = { startX: e.clientX, startWidth: panelWidth };
          document.body.style.cursor = "col-resize";
        }}
      />
    );
  }

  if (!node) {
    return null;
  }

  const visibleNodeIds = new Set([
    ...visibleEdges.map((e) => e.source),
    ...visibleEdges.map((e) => e.target),
  ]);

  // True if nodeId is visible on the graph, or a visible ancestor represents it
  const isEndpointVisible = (nodeId: string): boolean => {
    if (visibleNodeIds.has(nodeId)) return true;
    for (const visibleId of visibleNodeIds) {
      if (nodeId.startsWith(visibleId + ".")) return true;
    }
    return false;
  };

  // True if nodeId is the selected node or one of its descendants
  const isSelectedOrDescendant = (nodeId: string): boolean =>
    nodeId === node.id || nodeId.startsWith(node.id + ".");

  const dependencies = allEdges
    .filter((edge) => isSelectedOrDescendant(edge.source) && isEndpointVisible(edge.target))
    .map((edge) => ({
      target: edge.target,
      type: edge.type,
    }))
    .filter(
      (dep, index, array) =>
        array.findIndex(
          (d) => d.target === dep.target && d.type === dep.type,
        ) === index,
    );

  const dependents = allEdges
    .filter((edge) => isSelectedOrDescendant(edge.target) && isEndpointVisible(edge.source))
    .map((edge) => ({
      source: edge.source,
      type: edge.type,
    }))
    .filter(
      (dep, index, array) =>
        array.findIndex(
          (d) => d.source === dep.source && d.type === dep.type,
        ) === index,
    );

  const getNodeName = (nodeId: string): string => {
    const foundNode = findNodeById(nodeId);
    return foundNode ? foundNode.id : nodeId;
  };

  const getEdgeTypeBadge = (edgeType: "calls" | "is-a" | "has-a") => {
    const styles = {
      calls: "bg-blue-100 text-blue-800 border-blue-200",
      "is-a": "bg-green-100 text-green-800 border-green-200",
      "has-a": "bg-orange-100 text-orange-800 border-orange-200",
    };
    const labels = {
      calls: "Calls",
      "is-a": "Inherits",
      "has-a": "Contains",
    };
    return (
      <span
        className={`inline-flex px-2 py-0.5 text-xs font-medium rounded border ${styles[edgeType]}`}
      >
        {labels[edgeType]}
      </span>
    );
  };

  const handleDependencyClick = (nodeId: string) => {
    const targetNode = findNodeById(nodeId);
    if (targetNode) {
      expandParentNodes(nodeId);
      selectNode(nodeId);
    }
  };

  const formatPosition = (pos: [number, number] | null | undefined): string => {
    if (!pos || (pos[0] === 0 && pos[1] === 0)) {
      return "Not available";
    }
    return `Lines ${pos[0]} - ${pos[1]}`;
  };

  return (
    <div
      style={{ width: panelWidth }}
      className="h-full bg-white dark:bg-slate-900 border-r border-gray-100 dark:border-slate-700 flex flex-col relative"
    >
      <div
        role="separator"
        aria-orientation="vertical"
        onMouseDown={(e) => {
          e.preventDefault();
          dragState.current = { startX: e.clientX, startWidth: panelWidth };
          document.body.style.cursor = "col-resize";
        }}
        className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize z-40 hover:bg-blue-400/40 transition-colors"
      />

      <div className="flex-1 min-h-0 overflow-y-auto p-4 space-y-4">
        <div>
          <div className="flex items-center gap-2 mb-2">
            <span
              className={`inline-flex px-2 py-0.5 text-xs font-medium rounded-full ${
                node.type === "module"
                  ? "bg-blue-100 text-blue-800"
                  : node.type === "class"
                    ? "bg-green-100 text-green-800"
                    : node.type === "function"
                      ? "bg-purple-100 text-purple-800"
                      : "bg-gray-100 dark:bg-slate-700 text-gray-800 dark:text-gray-200"
              }`}
            >
              {node.type || "unknown"}
            </span>
            {(node.changeStatus || node.isGhost) && (
              <span
                className={`inline-flex px-2 py-0.5 text-xs font-semibold rounded-full ${
                  node.isGhost || node.changeStatus === "deleted"
                    ? "bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-400"
                    : node.changeStatus === "modified"
                      ? "bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-400"
                      : "bg-sky-100 text-sky-700 dark:bg-sky-900/40 dark:text-sky-400"
                }`}
              >
                {node.isGhost || node.changeStatus === "deleted" ? "deleted" : node.changeStatus}
              </span>
            )}
          </div>
          <p
            className="overflow-x-auto text-sm text-gray-900 dark:text-gray-100 font-mono bg-gray-50 dark:bg-slate-800 px-3 py-2 rounded border border-gray-200 dark:border-slate-700 text-ellipsis whitespace-nowrap text-right"
            style={{ direction: "rtl", textAlign: "left" }}
          >
            <span style={{ direction: "ltr", unicodeBidi: "bidi-override" }}>
              {node.id}
            </span>
          </p>
        </div>

        {node.children && node.children.length > 0 && (
          <div>
            <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1 uppercase tracking-wide">
              Children ({node.children.length})
            </h3>
            <div className="space-y-1 max-h-40 overflow-y-auto border border-gray-200 dark:border-slate-700 rounded bg-gray-50 dark:bg-slate-800 p-2">
              {node.children.map((child) => (
                <button
                  key={child.id}
                  onClick={() => handleDependencyClick(child.id)}
                  className="w-full flex items-center justify-between text-left text-sm text-gray-600 dark:text-gray-300 bg-white dark:bg-slate-700 px-3 py-1.5 rounded border border-gray-200 dark:border-slate-600 font-mono hover:bg-blue-50 hover:text-blue-700 hover:border-blue-300 dark:hover:bg-slate-600 dark:hover:text-white transition-colors cursor-pointer"
                >
                  <span className="truncate">
                    {child.id.split(".").pop() || child.id}
                  </span>
                  <span
                    className={`inline-flex px-1.5 py-0.5 text-[10px] font-medium rounded-full ${
                      child.type === "module"
                        ? "bg-blue-100 text-blue-800"
                        : child.type === "class"
                          ? "bg-green-100 text-green-800"
                          : child.type === "function"
                            ? "bg-purple-100 text-purple-800"
                            : "bg-gray-100 text-gray-800"
                    }`}
                  >
                    {child.type || "unknown"}
                  </span>
                </button>
              ))}
            </div>
          </div>
        )}

        {node.line_positions && !(node.line_positions[0] === 0 && node.line_positions[1] === 0) && (
          <div className="text-xs text-gray-500 dark:text-gray-400">
            {formatPosition(node.line_positions)}
          </div>
        )}

        {node.docstring && (
          <div>
            <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1 uppercase tracking-wide">
              Documentation
            </h3>
            <div className="bg-gray-50 dark:bg-slate-800 px-3 py-2 rounded border border-gray-200 dark:border-slate-700">
              <pre className="text-sm text-gray-700 dark:text-gray-200 whitespace-pre-wrap font-mono">
                {node.docstring}
              </pre>
            </div>
          </div>
        )}

        {node.code_block && (
          <div>
            <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1 uppercase tracking-wide">
              Source Code
            </h3>
            <div className="rounded-lg overflow-auto max-h-80 relative">
              <pre className={`language-${getPrismLanguage(node)}`}>
                <code
                  ref={codeRef}
                  className={`language-${getPrismLanguage(node)}`}
                  style={{ fontSize: "12px", lineHeight: 1.4 }}
                >
                  {node.code_block}
                </code>
              </pre>
              <div className="absolute top-2 right-0 m-2">
                <button
                  onClick={() => setShowCodeModal(true)}
                  className="text-xs p-2 bg-white/80 dark:bg-slate-700/60 border dark:border-slate-600 rounded text-gray-600 dark:text-gray-200 hover:bg-white dark:hover:bg-slate-600"
                >
                  <IconArrowsMaximize size={14} strokeWidth={1.5} />
                </button>
              </div>
            </div>
          </div>
        )}

        {showCodeModal && (
          <div className="fixed inset-0 z-60 flex items-center justify-center bg-black/50 dark:bg-black/70">
            <div className="bg-white dark:bg-slate-800 rounded-lg shadow-xl max-w-4xl w-full max-h-[80vh] flex flex-col p-6 border border-gray-200 dark:border-slate-700">
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
                  Source Code - {node.id}
                </h3>
                <button
                  onClick={() => setShowCodeModal(false)}
                  className="px-3 py-1 bg-gray-100 dark:bg-slate-700 text-gray-800 dark:text-gray-200 rounded border border-gray-300 dark:border-slate-600 hover:bg-gray-200 dark:hover:bg-slate-600"
                >
                  Close
                </button>
              </div>
              <div className="flex items-center gap-2 mb-3">
                <input
                  ref={modalSearchRef}
                  type="text"
                  placeholder="Search… (Ctrl+F)"
                  value={modalSearchQuery}
                  onChange={(e) => setModalSearchQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && matchCount > 0) {
                      e.preventDefault();
                      if (e.shiftKey) {
                        setCurrentMatch(c => c <= 1 ? matchCount : c - 1);
                      } else {
                        setCurrentMatch(c => c >= matchCount ? 1 : c + 1);
                      }
                    }
                  }}
                  className="flex-1 text-sm px-3 py-1.5 border border-gray-300 dark:border-slate-600 rounded bg-white dark:bg-slate-700 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
                />
                {matchCount > 0 && (
                  <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
                    {currentMatch} / {matchCount}
                  </span>
                )}
                {modalSearchQuery.trim() && matchCount === 0 && (
                  <span className="text-xs text-red-500 whitespace-nowrap">No matches</span>
                )}
                <button
                  onClick={() => setCurrentMatch(c => c <= 1 ? matchCount : c - 1)}
                  disabled={matchCount === 0}
                  className="px-2 py-1 text-xs bg-gray-100 dark:bg-slate-700 text-gray-700 dark:text-gray-200 rounded border border-gray-300 dark:border-slate-600 hover:bg-gray-200 dark:hover:bg-slate-600 disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  ↑
                </button>
                <button
                  onClick={() => setCurrentMatch(c => c >= matchCount ? 1 : c + 1)}
                  disabled={matchCount === 0}
                  className="px-2 py-1 text-xs bg-gray-100 dark:bg-slate-700 text-gray-700 dark:text-gray-200 rounded border border-gray-300 dark:border-slate-600 hover:bg-gray-200 dark:hover:bg-slate-600 disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  ↓
                </button>
              </div>
              <pre className={`language-${getPrismLanguage(node)} bg-gray-50 dark:bg-slate-700 rounded p-3 overflow-auto flex-1 min-h-0`}>
                <code ref={modalCodeRef as any} className={`language-${getPrismLanguage(node)}`} />
              </pre>
            </div>
          </div>
        )}

        <div>
          <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1 uppercase tracking-wide">
            Depends On ({dependencies.length})
          </h3>
          {dependencies.length > 0 ? (
            <div className="space-y-1">
              {dependencies.map((dep) => (
                <button
                  key={`${dep.target}-${dep.type}`}
                  onClick={() => handleDependencyClick(dep.target)}
                  className="w-full text-left text-sm text-gray-600 dark:text-gray-300 bg-gray-50 dark:bg-slate-800 px-3 py-2 rounded border border-gray-200 dark:border-slate-700 font-mono hover:bg-blue-50 hover:text-blue-700 hover:border-blue-300 dark:hover:bg-slate-600 dark:hover:text-white transition-colors cursor-pointer"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate">{getNodeName(dep.target)}</span>
                    {getEdgeTypeBadge(dep.type)}
                  </div>
                </button>
              ))}
            </div>
          ) : (
            <p className="text-sm text-gray-500 italic">None</p>
          )}
        </div>

        <div>
          <h3 className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1 uppercase tracking-wide">
            Used By ({dependents.length})
          </h3>
          {dependents.length > 0 ? (
            <div className="space-y-1">
              {dependents.map((dep) => (
                <button
                  key={`${dep.source}-${dep.type}`}
                  onClick={() => handleDependencyClick(dep.source)}
                  className="w-full text-left text-sm text-gray-600 dark:text-gray-300 bg-gray-50 dark:bg-slate-800 px-3 py-2 rounded border border-gray-200 dark:border-slate-700 font-mono hover:bg-blue-50 hover:text-blue-700 hover:border-blue-300 dark:hover:bg-slate-600 dark:hover:text-white transition-colors cursor-pointer"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate">{getNodeName(dep.source)}</span>
                    {getEdgeTypeBadge(dep.type)}
                  </div>
                </button>
              ))}
            </div>
          ) : (
            <p className="text-sm text-gray-500 italic">None</p>
          )}
        </div>
      </div>
    </div>
  );
}

// ── CFG Detail Panel ─────────────────────────────────────────────────────────

interface CfgDetailContentProps {
  node: Node;
  onClose: () => void;
  onSelectNode: (id: string | null) => void;
  panelWidth: number;
  onResizeStart: (e: React.MouseEvent) => void;
}

function CfgDetailContent({
  node,
  onClose,
  onSelectNode,
  panelWidth,
  onResizeStart,
}: CfgDetailContentProps) {
  const shapeColors: Record<string, string> = {
    scope:
      "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300",
    call: "bg-blue-100 text-blue-800 dark:bg-blue-900/40 dark:text-blue-300",
    condition:
      "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300",
    statement:
      "bg-gray-100 text-gray-800 dark:bg-gray-900/40 dark:text-gray-300",
    variable:
      "bg-purple-100 text-purple-800 dark:bg-purple-900/40 dark:text-purple-300",
    literal: "bg-pink-100 text-pink-800 dark:bg-pink-900/40 dark:text-pink-300",
    merge:
      "bg-slate-100 text-slate-800 dark:bg-slate-900/40 dark:text-slate-300",
  };

  return (
    <div
      style={{ width: panelWidth }}
      className="h-full bg-white dark:bg-slate-900 border-r border-gray-100 dark:border-slate-700 flex flex-col relative"
      onClick={(e) => e.stopPropagation()}
    >
      <div
        role="separator"
        aria-orientation="vertical"
        onMouseDown={onResizeStart}
        className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize z-40 hover:bg-blue-400/40 transition-colors"
      />

      <div className="flex-1 min-h-0 overflow-y-auto space-y-4 p-4">
        <div className="flex items-center gap-2">
          <span
            className={`shrink-0 rounded px-2 py-0.5 text-xs font-semibold ${shapeColors[node.nodeShape || "statement"] ?? shapeColors.statement}`}
          >
            {node.nodeShape || "statement"}
          </span>
          <span
            className="truncate text-sm font-semibold text-gray-900 dark:text-gray-100"
            title={node.label || node.id}
          >
            {node.label || node.id}
          </span>
        </div>

        <div className="space-y-1 text-xs text-gray-500 dark:text-gray-400">
          <p>
            <span className="font-medium text-gray-700 dark:text-gray-300">
              ID:
            </span>{" "}
            <span className="font-mono">{node.id}</span>
          </p>
          {node.parent && (
            <p>
              <span className="font-medium text-gray-700 dark:text-gray-300">
                Scope:
              </span>{" "}
              <button
                onClick={() => onSelectNode(node.parent!)}
                className="font-mono text-blue-600 hover:underline dark:text-blue-400"
              >
                {node.parent}
              </button>
            </p>
          )}
          {node.collapsed && (
            <p className="italic text-amber-600 dark:text-amber-400">
              ▸ Collapsed — children hidden
            </p>
          )}
        </div>

        <div className="text-sm italic text-gray-500">
          Edge details will be available once CFG edge rendering is unified.
        </div>
      </div>
    </div>
  );
}
