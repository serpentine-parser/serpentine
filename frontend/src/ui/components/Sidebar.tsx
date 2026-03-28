import {
  IconArrowsMaximize,
  IconArrowsMinimize,
  IconBraces,
  IconChevronLeft,
  IconChevronRight,
  IconCircleDot,
  IconFile,
  IconFolder,
  IconFolderOpen,
  IconFunction,
  IconGitBranch,
  IconGitMerge,
  IconHierarchy2,
  IconLayoutGrid,
  IconPlayerPlay,
  IconRefresh,
  IconSearch,
  IconVariable,
  IconX,
} from "@tabler/icons-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Node } from "@domains/graph/model/types";

type ChangeStatusEntry = { changeStatus?: string; isGhost?: boolean };

interface TreeNodeProps {
  node: Node;
  depth: number;
  expandedNodes: Set<string>;
  onToggle: (nodeId: string) => void;
  onSelect: (nodeId: string) => void;
  selectedNodeId: string | null;
  searchQuery?: string;
  selectedRef?: React.RefObject<HTMLDivElement | null>;
  changeStatusLookup: Record<string, ChangeStatusEntry>;
  ghostChildrenByParent: Record<string, Node[]>;
}

interface HighlightedTextProps {
  text: string;
  highlight: string;
  className?: string;
}

function HighlightedText({
  text,
  highlight,
  className = "",
}: HighlightedTextProps) {
  if (!highlight.trim()) {
    return <span className={className}>{text}</span>;
  }

  const parts = text.split(
    new RegExp(`(${highlight.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi"),
  );

  return (
    <span className={className}>
      {parts.map((part, index) =>
        part.toLowerCase() === highlight.toLowerCase() ? (
          <mark
            key={index}
            className="bg-yellow-200 dark:bg-yellow-800 px-0.5 rounded"
          >
            {part}
          </mark>
        ) : (
          part
        ),
      )}
    </span>
  );
}

function TreeNode({
  node,
  depth,
  expandedNodes,
  onToggle,
  onSelect,
  selectedNodeId,
  searchQuery = "",
  selectedRef,
  changeStatusLookup,
  ghostChildrenByParent,
}: TreeNodeProps) {
  const hasChildren = node.children && node.children.length > 0;
  const isExpanded = expandedNodes.has(node.id);
  const isSelected = selectedNodeId === node.id;

  const getNodeIcon = (node: Node) => {
    if (node.nodeShape) {
      switch (node.nodeShape) {
        case "scope":
          return hasChildren ? (
            isExpanded ? (
              <IconFolderOpen size={16} />
            ) : (
              <IconFolder size={16} />
            )
          ) : (
            <IconFolder size={16} />
          );
        case "condition":
          return <IconGitBranch size={16} />;
        case "call":
          return <IconPlayerPlay size={16} />;
        case "variable":
          return <IconVariable size={16} />;
        case "merge":
          return <IconGitMerge size={16} />;
        case "statement":
          return <IconCircleDot size={16} />;
        default:
          return <IconFile size={16} />;
      }
    }
    switch (node.type) {
      case "module":
        return hasChildren ? (
          isExpanded ? (
            <IconFolderOpen size={16} />
          ) : (
            <IconFolder size={16} />
          )
        ) : (
          <IconFile size={16} />
        );
      case "class":
        return <IconBraces size={16} />;
      case "function":
        return <IconFunction size={16} />;
      default:
        return <IconFile size={16} />;
    }
  };

  const getOriginColor = (origin?: string) => {
    switch (origin) {
      case "local":
        return "text-blue-600 dark:text-blue-400";
      case "standard":
        return "text-gray-600 dark:text-gray-400";
      case "third-party":
        return "text-green-600 dark:text-green-400";
      default:
        return "text-gray-900 dark:text-gray-100";
    }
  };

  const getChangeStatusClasses = (node: Node) => {
    const live = changeStatusLookup[node.id];
    const isGhost = live?.isGhost ?? node.isGhost;
    const changeStatus = live?.changeStatus ?? node.changeStatus;
    if (isGhost || changeStatus === "deleted")
      return { dot: "bg-red-500", text: "text-red-500 dark:text-red-400 line-through opacity-60" };
    if (changeStatus === "modified")
      return { dot: "bg-amber-500", text: "text-amber-600 dark:text-amber-400" };
    if (changeStatus === "added")
      return { dot: "bg-sky-500", text: "text-sky-600 dark:text-sky-400" };
    return null;
  };

  return (
    <div>
      <div
        ref={isSelected ? selectedRef : undefined}
        className={`flex items-center gap-2 py-1 px-2 rounded cursor-pointer hover:bg-gray-100 dark:hover:bg-slate-800 ${
          isSelected
            ? "bg-blue-100 dark:bg-blue-900/30 text-blue-900 dark:text-blue-100"
            : ""
        }`}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
        onClick={() => onSelect(node.id)}
      >
        {hasChildren ? (
          <button
            className="flex-shrink-0 p-0.5 hover:bg-gray-200 dark:hover:bg-slate-700 rounded"
            onClick={(e) => {
              e.stopPropagation();
              onToggle(node.id);
            }}
          >
            <IconChevronRight
              size={14}
              className={`transform transition-transform ${isExpanded ? "rotate-90" : ""}`}
            />
          </button>
        ) : (
          <div className="w-5 h-5 flex-shrink-0" />
        )}

        {getChangeStatusClasses(node) && (
          <span className={`w-2 h-2 rounded-full flex-shrink-0 ${getChangeStatusClasses(node)!.dot}`} />
        )}
        <div className={`flex-shrink-0 ${getOriginColor(node.origin)}`}>
          {getNodeIcon(node)}
        </div>

        <HighlightedText
          text={node.id.split(".").pop() || node.id}
          highlight={searchQuery}
          className={`text-sm truncate ${getChangeStatusClasses(node)?.text ?? getOriginColor(node.origin)} ${isSelected ? "font-medium" : ""}`}
        />
      </div>

      {hasChildren && isExpanded && (
        <div>
          {node.children!.map((child) => (
            <TreeNode
              key={child.id}
              node={child}
              depth={depth + 1}
              expandedNodes={expandedNodes}
              onToggle={onToggle}
              onSelect={onSelect}
              selectedNodeId={selectedNodeId}
              searchQuery={searchQuery}
              selectedRef={selectedRef}
              changeStatusLookup={changeStatusLookup}
              ghostChildrenByParent={ghostChildrenByParent}
            />
          ))}
          {(ghostChildrenByParent[node.id] ?? []).filter((ghost) => !node.children?.some((c) => c.id === ghost.id)).map((ghost) => (
            <TreeNode
              key={ghost.id}
              node={ghost}
              depth={depth + 1}
              expandedNodes={expandedNodes}
              onToggle={onToggle}
              onSelect={onSelect}
              selectedNodeId={selectedNodeId}
              searchQuery={searchQuery}
              selectedRef={selectedRef}
              changeStatusLookup={changeStatusLookup}
              ghostChildrenByParent={ghostChildrenByParent}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export interface SidebarExpansionSignal {
  type: "expand" | "collapse";
  nodeId: string | null;
}

export interface SidebarProps {
  collapsed: boolean;
  onToggleCollapse: () => void;
  catalogNodes: Node[];
  selectedNodeId: string | null;
  selectNode: (id: string | null) => void;
  expandParentNodes: (id: string) => void;
  flipLayoutDirection: () => void;
  dismissAllChanges: () => void;
  graphNodes: Node[];
  expandAll: () => void;
  collapseAll: () => void;
  resetLayout: () => void;
  sidebarExpansionSignal: SidebarExpansionSignal | null;
  setSidebarExpansionSignal: (signal: SidebarExpansionSignal | null) => void;
}

export function Sidebar({
  collapsed,
  onToggleCollapse,
  catalogNodes,
  selectedNodeId,
  selectNode,
  expandParentNodes,
  flipLayoutDirection,
  dismissAllChanges,
  graphNodes,
  expandAll,
  collapseAll,
  resetLayout,
  sidebarExpansionSignal,
  setSidebarExpansionSignal,
}: SidebarProps) {
  const [expandedNodes, setExpandedNodes] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const selectedRowRef = useRef<HTMLDivElement | null>(null);

  const [sidebarWidth, setSidebarWidth] = useState(256);
  const dragState = useRef<{ startX: number; startWidth: number } | null>(null);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragState.current) return;
      const delta = e.clientX - dragState.current.startX;
      setSidebarWidth(Math.max(180, Math.min(500, dragState.current.startWidth + delta)));
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

  const hasChanges = useMemo(() => {
    const check = (nodeList: typeof graphNodes): boolean =>
      nodeList.some(
        (n) => n.changeStatus || n.isGhost || (n.children && check(n.children))
      );
    return check(graphNodes);
  }, [graphNodes]);

  const { changeStatusLookup, ghostChildrenByParent } = useMemo(() => {
    const lookup: Record<string, ChangeStatusEntry> = {};
    const ghosts: Record<string, Node[]> = {};
    const flatten = (nodes: typeof graphNodes) => {
      for (const node of nodes) {
        if (node.changeStatus || node.isGhost) {
          lookup[node.id] = { changeStatus: node.changeStatus ?? undefined, isGhost: node.isGhost };
        }
        if (node.isGhost && node.parent) {
          ghosts[node.parent] = [...(ghosts[node.parent] ?? []), node];
        }
        if (node.children) flatten(node.children);
      }
    };
    flatten(graphNodes);
    return { changeStatusLookup: lookup, ghostChildrenByParent: ghosts };
  }, [graphNodes]);

  const nodes = catalogNodes;
  const allNodeCount = catalogNodes.length;

  const autoExpandForSelection = useCallback(
    (nodeId: string) => {
      const ancestors: string[] = [];
      const parts = nodeId.split(".");
      for (let i = 1; i < parts.length; i++) {
        ancestors.push(parts.slice(0, i).join("."));
      }
      if (ancestors.length > 0) {
        setExpandedNodes((prev) => {
          const next = new Set(prev);
          for (const a of ancestors) {
            next.add(a);
          }
          return next;
        });
      }
    },
    [],
  );

  useEffect(() => {
    if (selectedNodeId) {
      autoExpandForSelection(selectedNodeId);
    }
  }, [selectedNodeId, autoExpandForSelection]);

  useEffect(() => {
    if (selectedNodeId && selectedRowRef.current) {
      const timer = setTimeout(() => {
        selectedRowRef.current?.scrollIntoView({
          behavior: "smooth",
          block: "nearest",
        });
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [selectedNodeId]);

  useEffect(() => {
    if (!sidebarExpansionSignal) return;
    const { type, nodeId } = sidebarExpansionSignal;

    if (type === "expand") {
      if (nodeId === null) {
        const allIds = new Set<string>();
        const collect = (nodeList: typeof nodes) => {
          nodeList.forEach((n) => { allIds.add(n.id); if (n.children) collect(n.children); });
        };
        collect(nodes);
        setExpandedNodes(allIds);
      } else {
        const subtreeIds = new Set<string>();
        const collectSubtree = (nodeList: typeof nodes, found: boolean) => {
          nodeList.forEach((n) => {
            if (found || n.id === nodeId) { subtreeIds.add(n.id); if (n.children) collectSubtree(n.children, true); }
            else if (n.children) collectSubtree(n.children, false);
          });
        };
        collectSubtree(nodes, false);
        setExpandedNodes((prev) => new Set([...prev, ...subtreeIds]));
      }
    } else {
      if (nodeId === null) {
        setExpandedNodes(new Set());
      } else {
        const subtreeIds = new Set<string>();
        const collectSubtree = (nodeList: typeof nodes, found: boolean) => {
          nodeList.forEach((n) => {
            if (found) { subtreeIds.add(n.id); if (n.children) collectSubtree(n.children, true); }
            else if (n.id === nodeId && n.children) collectSubtree(n.children, true);
            else if (n.children) collectSubtree(n.children, false);
          });
        };
        collectSubtree(nodes, false);
        setExpandedNodes((prev) => { const next = new Set(prev); subtreeIds.forEach((id) => next.delete(id)); return next; });
      }
    }

    setSidebarExpansionSignal(null);
  }, [sidebarExpansionSignal]); // eslint-disable-line react-hooks/exhaustive-deps

  const filteredNodes = useMemo(() => {
    if (!searchQuery.trim()) {
      return nodes;
    }

    const query = searchQuery.toLowerCase();

    const filterTree = (
      nodeList: Node[],
    ): { filteredNodes: Node[]; hasMatches: boolean } => {
      const filtered: Node[] = [];
      let anyMatches = false;

      nodeList.forEach((node) => {
        const matchText = (node.label || node.id).toLowerCase();
        const nodeParts = node.id.split(".");
        const nodeMatches =
          matchText.includes(query) ||
          nodeParts.some((part) => part.toLowerCase().startsWith(query));

        const childResult = node.children
          ? filterTree(node.children)
          : { filteredNodes: undefined, hasMatches: false };

        if (nodeMatches || childResult.hasMatches) {
          filtered.push({
            ...node,
            children: childResult.filteredNodes,
          });
          anyMatches = true;
        }
      });

      return { filteredNodes: filtered, hasMatches: anyMatches };
    };

    return filterTree(nodes).filteredNodes;
  }, [nodes, searchQuery]);

  useMemo(() => {
    if (searchQuery.trim()) {
      const nodesToExpand = new Set<string>();
      const query = searchQuery.toLowerCase();

      const determineExpansion = (nodeList: Node[]) => {
        nodeList.forEach((node) => {
          const matchText = (node.label || node.id).toLowerCase();
          const nodeParts = node.id.split(".");
          const nodeMatches =
            matchText.includes(query) ||
            nodeParts.some((part) => part.toLowerCase().startsWith(query));

          if (node.children && node.children.length > 0) {
            const hasVisibleChildren = node.children.length > 0;

            if (hasVisibleChildren && !nodeMatches) {
              nodesToExpand.add(node.id);
            }

            determineExpansion(node.children);
          }
        });
      };

      determineExpansion(filteredNodes);
      setExpandedNodes(nodesToExpand);
    }
  }, [searchQuery, filteredNodes]);

  const handleToggleNode = (nodeId: string) => {
    setExpandedNodes((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(nodeId)) {
        newSet.delete(nodeId);
      } else {
        newSet.add(nodeId);
      }
      return newSet;
    });
  };

  const handleSelectNode = (nodeId: string) => {
    expandParentNodes(nodeId);
    selectNode(nodeId);
  };

  const clearSearch = () => {
    setSearchQuery("");
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && searchQuery) {
        clearSearch();
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "l") {
        e.preventDefault();
        flipLayoutDirection();
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "f" && !collapsed) {
        e.preventDefault();
        const searchInput = document.querySelector(
          "#node-search",
        ) as HTMLInputElement;
        if (searchInput) {
          searchInput.focus();
          searchInput.select();
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [searchQuery, collapsed, flipLayoutDirection]);

  const topLevelNodes = filteredNodes.filter((node) => !node.parent);

  const visibleNodesCount = useMemo(() => {
    if (!searchQuery.trim()) return nodes.length;

    const countVisibleNodes = (nodeList: Node[]): number => {
      let count = 0;
      nodeList.forEach((node) => {
        count++;
        if (
          expandedNodes.has(node.id) &&
          node.children &&
          node.children.length > 0
        ) {
          count += countVisibleNodes(node.children);
        }
      });
      return count;
    };

    return countVisibleNodes(topLevelNodes);
  }, [topLevelNodes, expandedNodes, searchQuery, nodes.length]);

  return (
    <aside
      style={collapsed ? { width: 32 } : { width: sidebarWidth }}
      className="relative bg-white dark:bg-slate-900 border-r border-gray-100 dark:border-slate-700 shadow-sm hidden md:flex flex-col flex-shrink-0 overflow-hidden"
    >
      {!collapsed && (
        <div
          role="separator"
          aria-orientation="vertical"
          onMouseDown={(e) => {
            e.preventDefault();
            dragState.current = { startX: e.clientX, startWidth: sidebarWidth };
            document.body.style.cursor = "col-resize";
          }}
          className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize z-40 hover:bg-blue-400/40 transition-colors"
        />
      )}
      <div className="flex items-center justify-between h-16 px-4">
        {!collapsed ? (
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Object Explorer
          </h2>
        ) : null}

        {!collapsed && (
          <div className="flex items-center gap-1 ml-auto mr-2">
            <button
              onClick={expandAll}
              className="p-1 text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 rounded transition-colors"
              title="Expand all"
            >
              <IconArrowsMaximize size={14} />
            </button>
            <button
              onClick={collapseAll}
              className="p-1 text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 rounded transition-colors"
              title="Collapse all"
            >
              <IconArrowsMinimize size={14} />
            </button>
            <button
              onClick={resetLayout}
              className="p-1 text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 rounded transition-colors"
              title="Reset layout"
            >
              <IconLayoutGrid size={14} />
            </button>
          </div>
        )}

        <button
          onClick={onToggleCollapse}
          className="flex-shrink-0 bg-white dark:bg-slate-800 border border-gray-100 dark:border-slate-700 shadow-sm w-8 h-8 rounded-full flex items-center justify-center text-gray-700 dark:text-gray-200 hover:scale-105 transition-transform"
          aria-label="Toggle navigation"
        >
          {collapsed ? (
            <IconChevronRight size={16} strokeWidth={1.5} />
          ) : (
            <IconChevronLeft size={16} strokeWidth={1.5} />
          )}
        </button>
      </div>

      {!collapsed && (
        <>
          <div className="px-6 py-2 text-sm text-gray-500 dark:text-gray-400 border-b border-gray-100 dark:border-slate-700 flex items-center justify-between">
            <span>All objects ({allNodeCount})</span>
            {hasChanges && (
              <button
                onClick={dismissAllChanges}
                className="flex items-center gap-1 text-xs text-amber-600 dark:text-amber-400 hover:text-amber-800 dark:hover:text-amber-200 transition-colors"
                title="Clear all change indicators"
              >
                <IconRefresh size={12} />
                Clear changes
              </button>
            )}
          </div>

          <div className="px-4 py-3 border-b border-gray-100 dark:border-slate-700">
            <div className="relative">
              <IconSearch
                size={16}
                className="absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 dark:text-gray-500"
              />
              <input
                id="node-search"
                type="text"
                placeholder="Search nodes... (Ctrl+F)"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-9 pr-8 py-2 text-sm bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-600 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-gray-900 dark:text-gray-100"
              />
              {searchQuery && (
                <button
                  onClick={clearSearch}
                  className="absolute right-2 top-1/2 transform -translate-y-1/2 text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300"
                >
                  <IconX size={14} />
                </button>
              )}
            </div>
            {searchQuery && (
              <div className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                {topLevelNodes.length === 0
                  ? "No matches found"
                  : `${visibleNodesCount} visible nodes match "${searchQuery}"`}
              </div>
            )}
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto">
            <div className="py-2">
              {topLevelNodes.length === 0 ? (
                <div className="px-4 py-8 text-center text-gray-500 dark:text-gray-400">
                  {searchQuery ? (
                    <div>
                      <IconSearch
                        size={24}
                        className="mx-auto mb-2 opacity-50"
                      />
                      <p className="text-sm">No nodes match "{searchQuery}"</p>
                      <button
                        onClick={clearSearch}
                        className="mt-2 text-xs text-blue-600 dark:text-blue-400 hover:underline"
                      >
                        Clear search
                      </button>
                    </div>
                  ) : (
                    <div>
                      <IconFolder
                        size={24}
                        className="mx-auto mb-2 opacity-50"
                      />
                      <p className="text-sm">No nodes available</p>
                    </div>
                  )}
                </div>
              ) : (
                topLevelNodes.map((node) => (
                  <TreeNode
                    key={node.id}
                    node={node}
                    depth={0}
                    expandedNodes={expandedNodes}
                    onToggle={handleToggleNode}
                    onSelect={handleSelectNode}
                    selectedNodeId={selectedNodeId}
                    searchQuery={searchQuery}
                    selectedRef={selectedRowRef}
                    changeStatusLookup={changeStatusLookup}
                    ghostChildrenByParent={ghostChildrenByParent}
                  />
                ))
              )}
            </div>
          </div>
        </>
      )}
    </aside>
  );
}
