import {
  IconCamera,
  IconDownload,
  IconSearch,
  IconSettings,
  IconX,
} from "@tabler/icons-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Node } from "@domains/graph/model/types";
import type { CfgEdgeData } from "@domains/graph/model/types";

interface SearchBarProps {
  searchQuery: string;
  setSearchQuery: (q: string) => void;
  excludeQuery: string;
  setExcludeQuery: (q: string) => void;
  executeSearch: () => void;
  exportPng: () => Promise<void>;
  nodes: Node[];
  catalogNodes: Node[];
  pdgVisibleEdgeTypes: Set<string>;
  setPdgVisibleEdgeTypes: (types: Set<string>) => void;
  pdgAllEdges: CfgEdgeData[];
  includeStandardPackages: boolean;
  includeThirdPartyPackages: boolean;
  setIncludeStandardPackages: (v: boolean) => void;
  setIncludeThirdPartyPackages: (v: boolean) => void;
  visibleEdgeDepth: number;
  setVisibleEdgeDepth: (v: number) => void;
  selectorState: string;
  setStateFilter: (state: string) => void;
  onExportJson: () => void;
}

export function SearchBar({
  searchQuery,
  setSearchQuery,
  excludeQuery,
  setExcludeQuery,
  executeSearch,
  exportPng,
  nodes,
  catalogNodes,
  pdgVisibleEdgeTypes,
  setPdgVisibleEdgeTypes,
  pdgAllEdges,
  includeStandardPackages,
  includeThirdPartyPackages,
  setIncludeStandardPackages,
  setIncludeThirdPartyPackages,
  visibleEdgeDepth,
  setVisibleEdgeDepth,
  selectorState,
  setStateFilter,
  onExportJson,
}: SearchBarProps) {
  const [showDropdown, setShowDropdown] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const [showExcludeDropdown, setShowExcludeDropdown] = useState(false);
  const excludeInputRef = useRef<HTMLInputElement>(null);
  const [showSettings, setShowSettings] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);

  const [isSearching, setIsSearching] = useState(false);

  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [excludeSelectedIndex, setExcludeSelectedIndex] = useState(-1);

  const dropdownOpenRef = useRef(false);
  const excludeDropdownOpenRef = useRef(false);

  const [dropdownPosition, setDropdownPosition] = useState<"top" | "bottom">("bottom");
  const [excludeDropdownPosition, setExcludeDropdownPosition] = useState<"top" | "bottom">("bottom");
  const searchInputContainerRef = useRef<HTMLDivElement>(null);
  const excludeInputContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      const target = event.target as Element;

      if (settingsRef.current && !settingsRef.current.contains(target)) {
        setShowSettings(false);
      }

      if (inputRef.current && !inputRef.current.contains(target)) {
        const dropdown = inputRef.current.parentElement?.querySelector(
          '[data-role="search-dropdown"]',
        );
        if (!dropdown || !dropdown.contains(target)) {
          setShowDropdown(false);
        }
      }

      if (
        excludeInputRef.current &&
        !excludeInputRef.current.contains(target)
      ) {
        const dropdown = excludeInputRef.current.parentElement?.querySelector(
          '[data-role="exclude-dropdown"]',
        );
        if (!dropdown || !dropdown.contains(target)) {
          setShowExcludeDropdown(false);
        }
      }
    }

    if (showSettings || showDropdown || showExcludeDropdown) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [showSettings, showDropdown, showExcludeDropdown]);

  const hotkeyLabel =
    typeof navigator !== "undefined" &&
    /Mac|iP(hone|ad|od)|MacIntel/.test(navigator.platform)
      ? "⌘E"
      : "Ctrl+E";

  const parseQueryTerm = (query: string): { terms: string[]; lastTerm: string; cleanTerm: string } => {
    const terms = query.trim().split(",");
    const lastTerm = terms[terms.length - 1]?.trim() || "";
    const cleanTerm = lastTerm.toLowerCase().replace(/[@+|*]/g, "");
    return { terms, lastTerm, cleanTerm };
  };

  const getOperatorsFromTerm = (term: string): { leading: string; trailing: string } => {
    const leading = term.match(/^[@+|*]+/)?.[0] || "";
    const trailing = term.match(/[+|*]+$/)?.[0] || "";
    return { leading, trailing };
  };

  const determineDropdownPosition = (ref: React.RefObject<HTMLDivElement>): "top" | "bottom" => {
    if (!ref.current) return "bottom";
    const rect = ref.current.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    const spaceAbove = rect.top;
    return spaceBelow < 250 && spaceAbove > 250 ? "top" : "bottom";
  };

  const flattenNodeNames = (nodeList: Node[]): string[] => {
    let names: string[] = [];
    nodeList.forEach((node: Node) => {
      names.push(node.id);
      if (node.children) {
        names = names.concat(flattenNodeNames(node.children));
      }
    });
    return names;
  };

  const partialMatches = useMemo(() => {
    if (!searchQuery.trim()) return [];

    const { cleanTerm } = parseQueryTerm(searchQuery);
    if (!cleanTerm) return [];

    return flattenNodeNames(catalogNodes).filter((name: string) =>
      name.toLowerCase().includes(cleanTerm),
    );
  }, [searchQuery, catalogNodes]);

  const excludeMatches = useMemo(() => {
    if (!excludeQuery.trim()) return [];

    const { cleanTerm } = parseQueryTerm(excludeQuery);
    if (!cleanTerm) return [];

    return flattenNodeNames(catalogNodes).filter((name: string) =>
      name.toLowerCase().includes(cleanTerm),
    );
  }, [excludeQuery, catalogNodes]);

  const getTotalNodeCount = (nodeList: typeof nodes): number => {
    let count = 0;
    const countRecursive = (nodes: typeof nodeList) => {
      nodes.forEach((node) => {
        count++;
        if (node.children) {
          countRecursive(node.children);
        }
      });
    };
    countRecursive(nodeList);
    return count;
  };

  const handleSelect = (name: string) => {
    const { terms, lastTerm } = parseQueryTerm(searchQuery);
    const { leading, trailing } = getOperatorsFromTerm(lastTerm);

    const newLastTerm = leading + name + trailing;

    const newTerms = [...terms.slice(0, -1), newLastTerm];
    const newQuery = newTerms.join(",");

    setSearchQuery(newQuery);
    setShowDropdown(false);
    setSelectedIndex(-1);
    dropdownOpenRef.current = false;
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  const handleExcludeSelect = (name: string) => {
    const { terms, lastTerm } = parseQueryTerm(excludeQuery);
    const { leading, trailing } = getOperatorsFromTerm(lastTerm);

    const newLastTerm = leading + name + trailing;

    const newTerms = [...terms.slice(0, -1), newLastTerm];
    const newQuery = newTerms.join(",");

    setExcludeQuery(newQuery);
    setShowExcludeDropdown(false);
    setExcludeSelectedIndex(-1);
    excludeDropdownOpenRef.current = false;
    setTimeout(() => excludeInputRef.current?.focus(), 0);
  };

  const highlightMatch = (text: string, query: string) => {
    if (!query) return text;

    const { cleanTerm } = parseQueryTerm(query);
    if (!cleanTerm) return text;

    const index = text.toLowerCase().indexOf(cleanTerm.toLowerCase());
    if (index === -1) return text;

    return (
      <>
        {text.substring(0, index)}
        <span className="bg-yellow-200 dark:bg-yellow-800 font-medium">
          {text.substring(index, index + cleanTerm.length)}
        </span>
        {text.substring(index + cleanTerm.length)}
      </>
    );
  };

  const modeColor = "bg-emerald-200/30 dark:bg-emerald-900/30";
  const modeBorder = "border-emerald-200/30 dark:border-emerald-700";
  const modeButtonColor = "bg-emerald-900 dark:bg-emerald-800";
  const modeIconColor = "text-emerald-400 dark:text-emerald-300";

  const hasPendingSearch = searchQuery.trim() || excludeQuery.trim();

  return (
    <div className="w-full">
      <div
        className={`backdrop-blur ${modeBorder} ${modeColor} rounded-xl shadow-lg p-3 flex items-center gap-3`}
      >
        <div className="flex-1 flex gap-2 items-center relative">
          <div ref={searchInputContainerRef} className="relative flex items-center bg-white dark:bg-slate-800 border border-gray-100 dark:border-slate-600 rounded-lg px-3 py-2 shadow-sm w-full">
            <IconSearch
              size={18}
              strokeWidth={1.75}
              className={`${modeIconColor} mr-2`}
            />
            <input
              ref={inputRef}
              type="text"
              placeholder="e.g., models.* or +my_module+ or @core"
              value={searchQuery}
              disabled={isSearching}
              onChange={(e) => {
                setSearchQuery((e.target as HTMLInputElement).value);
                dropdownOpenRef.current = true;
                setDropdownPosition(determineDropdownPosition(searchInputContainerRef));
                setShowDropdown(true);
                setSelectedIndex(-1);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  if (selectedIndex >= 0 && partialMatches[selectedIndex]) {
                    handleSelect(partialMatches[selectedIndex]);
                  } else {
                    setIsSearching(true);
                    try {
                      executeSearch();
                    } finally {
                      setIsSearching(false);
                    }
                    setShowDropdown(false);
                    dropdownOpenRef.current = false;
                    inputRef.current?.blur();
                  }
                } else if (e.key === "ArrowDown") {
                  e.preventDefault();
                  dropdownOpenRef.current = true;
                  setDropdownPosition(determineDropdownPosition(searchInputContainerRef));
                  setShowDropdown(true);
                  setSelectedIndex((prev) =>
                    prev < Math.min(partialMatches.length - 1, 4)
                      ? prev + 1
                      : prev,
                  );
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  dropdownOpenRef.current = true;
                  setDropdownPosition(determineDropdownPosition(searchInputContainerRef));
                  setShowDropdown(true);
                  setSelectedIndex((prev) => (prev > -1 ? prev - 1 : -1));
                } else if (e.key === "Escape") {
                  setShowDropdown(false);
                  dropdownOpenRef.current = false;
                  setSelectedIndex(-1);
                  inputRef.current?.blur();
                }
              }}
              onFocus={() => {
                if (searchQuery && partialMatches.length > 0) {
                  dropdownOpenRef.current = true;
                  setDropdownPosition(determineDropdownPosition(searchInputContainerRef));
                  setShowDropdown(true);
                  setSelectedIndex(-1);
                }
              }}
              onBlur={() => {
                setTimeout(() => {
                  if (!dropdownOpenRef.current) {
                    setShowDropdown(false);
                    setSelectedIndex(-1);
                  }
                }, 100);
              }}
              className="w-full bg-transparent outline-none text-sm text-gray-900 dark:text-gray-100 placeholder-gray-500 dark:placeholder-gray-400 disabled:opacity-60"
            />

            {showDropdown && partialMatches.length > 0 && (
              <div
                data-role="search-dropdown"
                className={`absolute left-0 right-0 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg shadow-lg max-h-60 overflow-y-auto z-50 ${
                  dropdownPosition === "bottom"
                    ? "top-full mt-1"
                    : "bottom-full mb-1"
                }`}
                onMouseEnter={() => {
                  dropdownOpenRef.current = true;
                }}
                onMouseLeave={() => {
                  dropdownOpenRef.current = false;
                }}
              >
                {partialMatches.slice(0, 5).map((match, i) => (
                  <div
                    key={i}
                    className={`px-3 py-2 cursor-pointer text-sm text-gray-900 dark:text-gray-100 ${
                      i === selectedIndex
                        ? "bg-blue-100 dark:bg-blue-900/30"
                        : "hover:bg-gray-100 dark:hover:bg-slate-700"
                    }`}
                    onClick={() => {
                      dropdownOpenRef.current = false;
                      handleSelect(match);
                    }}
                    onMouseEnter={() => setSelectedIndex(i)}
                  >
                    <span
                      className="truncate block"
                      style={{ direction: "rtl", textAlign: "left" }}
                    >
                      {highlightMatch(match, searchQuery)}
                    </span>
                  </div>
                ))}
                {partialMatches.length > 5 && (
                  <div className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400 border-t border-gray-200 dark:border-slate-600">
                    +{partialMatches.length - 5} more matches
                  </div>
                )}
              </div>
            )}
          </div>

          <div ref={excludeInputContainerRef} className="relative flex items-center bg-white dark:bg-slate-800 border border-gray-100 dark:border-slate-600 rounded-lg px-3 py-2 shadow-sm">
            <IconX
              size={18}
              strokeWidth={1.75}
              className={`${modeIconColor} mr-2`}
            />
            <input
              ref={excludeInputRef}
              type="text"
              placeholder="e.g., test* or @external"
              value={excludeQuery}
              disabled={isSearching}
              onChange={(e) => {
                setExcludeQuery((e.target as HTMLInputElement).value);
                excludeDropdownOpenRef.current = true;
                setExcludeDropdownPosition(determineDropdownPosition(excludeInputContainerRef));
                setShowExcludeDropdown(true);
                setExcludeSelectedIndex(-1);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  if (
                    excludeSelectedIndex >= 0 &&
                    excludeMatches[excludeSelectedIndex]
                  ) {
                    handleExcludeSelect(excludeMatches[excludeSelectedIndex]);
                  } else {
                    setIsSearching(true);
                    try {
                      executeSearch();
                    } finally {
                      setIsSearching(false);
                    }
                    setShowExcludeDropdown(false);
                    excludeDropdownOpenRef.current = false;
                    excludeInputRef.current?.blur();
                  }
                } else if (e.key === "ArrowDown") {
                  e.preventDefault();
                  excludeDropdownOpenRef.current = true;
                  setExcludeDropdownPosition(determineDropdownPosition(excludeInputContainerRef));
                  setShowExcludeDropdown(true);
                  setExcludeSelectedIndex((prev) =>
                    prev < Math.min(excludeMatches.length - 1, 4)
                      ? prev + 1
                      : prev,
                  );
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  excludeDropdownOpenRef.current = true;
                  setExcludeDropdownPosition(determineDropdownPosition(excludeInputContainerRef));
                  setShowExcludeDropdown(true);
                  setExcludeSelectedIndex((prev) =>
                    prev > -1 ? prev - 1 : -1,
                  );
                } else if (e.key === "Escape") {
                  setShowExcludeDropdown(false);
                  excludeDropdownOpenRef.current = false;
                  setExcludeSelectedIndex(-1);
                  excludeInputRef.current?.blur();
                }
              }}
              onFocus={() => {
                if (excludeQuery && excludeMatches.length > 0) {
                  excludeDropdownOpenRef.current = true;
                  setExcludeDropdownPosition(determineDropdownPosition(excludeInputContainerRef));
                  setShowExcludeDropdown(true);
                  setExcludeSelectedIndex(-1);
                }
              }}
              onBlur={() => {
                setTimeout(() => {
                  if (!excludeDropdownOpenRef.current) {
                    setShowExcludeDropdown(false);
                    setExcludeSelectedIndex(-1);
                  }
                }, 100);
              }}
              className="w-48 bg-transparent outline-none text-sm text-gray-900 dark:text-gray-100 placeholder-gray-500 dark:placeholder-gray-400 disabled:opacity-60"
            />

            {showExcludeDropdown && excludeMatches.length > 0 && (
              <div
                data-role="exclude-dropdown"
                className={`absolute left-0 right-0 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg shadow-lg max-h-60 overflow-y-auto z-50 ${
                  excludeDropdownPosition === "bottom"
                    ? "top-full mt-1"
                    : "bottom-full mb-1"
                }`}
                onMouseEnter={() => {
                  excludeDropdownOpenRef.current = true;
                }}
                onMouseLeave={() => {
                  excludeDropdownOpenRef.current = false;
                }}
              >
                {excludeMatches.slice(0, 5).map((match, i) => (
                  <div
                    key={i}
                    className={`px-3 py-2 cursor-pointer text-sm text-gray-900 dark:text-gray-100 ${
                      i === excludeSelectedIndex
                        ? "bg-blue-100 dark:bg-blue-900/30"
                        : "hover:bg-gray-100 dark:hover:bg-slate-700"
                    }`}
                    onClick={() => {
                      excludeDropdownOpenRef.current = false;
                      handleExcludeSelect(match);
                    }}
                    onMouseEnter={() => setExcludeSelectedIndex(i)}
                  >
                    <span
                      className="truncate block"
                      style={{ direction: "rtl", textAlign: "left" }}
                    >
                      {highlightMatch(match, excludeQuery)}
                    </span>
                  </div>
                ))}
                {excludeMatches.length > 5 && (
                  <div className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400 border-t border-gray-200 dark:border-slate-600">
                    +{excludeMatches.length - 5} more matches
                  </div>
                )}
              </div>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2">
          {hasPendingSearch && (
            <button
              onClick={() => {
                setSearchQuery("");
                setExcludeQuery("");
                setIsSearching(true);
                try {
                  executeSearch();
                } finally {
                  setIsSearching(false);
                }
              }}
              disabled={isSearching}
              className="bg-gray-500 dark:bg-gray-600 text-white px-3 py-2 rounded-md text-sm flex items-center gap-1 hover:opacity-90 transition-all disabled:opacity-60"
              title="Clear search and show all nodes"
            >
              <IconX size={14} strokeWidth={1.75} />
              <span>Clear</span>
            </button>
          )}
          <button
            className={`${
              isSearching
                ? "opacity-60 cursor-wait"
                : hasPendingSearch
                  ? "bg-blue-600 dark:bg-blue-500 animate-pulse"
                  : modeButtonColor
            } text-white px-4 py-2 rounded-md text-sm flex items-center gap-2 hover:opacity-90 transition-all disabled:opacity-60`}
            onClick={() => {
              setIsSearching(true);
              try {
                executeSearch();
              } finally {
                setIsSearching(false);
              }
            }}
            disabled={isSearching}
            title={
              isSearching
                ? "Searching..."
                : hasPendingSearch
                  ? "Click to apply search filters"
                  : "Search using selector syntax"
            }
          >
            <IconSearch size={16} strokeWidth={1.75} />
            <span>{isSearching ? "Searching..." : "Search"}</span>
          </button>

          <div className="relative" ref={settingsRef}>
            <button
              onClick={() => setShowSettings(!showSettings)}
              className="relative z-50 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-600 p-2 rounded-md shadow-sm"
              aria-label="Search settings"
            >
              <IconSettings
                size={18}
                strokeWidth={1.75}
                className="text-gray-600 dark:text-gray-300"
              />
            </button>

            {showSettings && (
              <div className="absolute right-0 bottom-full mb-2 w-64 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-lg shadow-lg p-3 z-50">
                <div className="flex items-center justify-between mb-3">
                  <div className="text-sm font-medium text-gray-700 dark:text-gray-200">
                    Settings
                  </div>
                  <button
                    onClick={() => setShowSettings(false)}
                    className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-slate-700 transition-colors"
                    aria-label="Close settings"
                  >
                    <IconX
                      size={16}
                      strokeWidth={1.5}
                      className="text-gray-400 dark:text-gray-500"
                    />
                  </button>
                </div>

                <label className="flex items-center justify-between mb-2">
                      <span className="text-sm text-gray-700 dark:text-gray-200">
                        Include standard library
                      </span>
                      <input
                        type="checkbox"
                        checked={includeStandardPackages}
                        onChange={(e) =>
                          setIncludeStandardPackages(
                            (e.target as HTMLInputElement).checked,
                          )
                        }
                        className="ml-2 rounded border-gray-300 dark:border-slate-600 text-blue-600 focus:ring-blue-500 dark:bg-slate-700"
                      />
                    </label>
                    <label className="flex items-center justify-between mb-3">
                      <span className="text-sm text-gray-700 dark:text-gray-200">
                        Include third-party
                      </span>
                      <input
                        type="checkbox"
                        checked={includeThirdPartyPackages}
                        onChange={(e) =>
                          setIncludeThirdPartyPackages(
                            (e.target as HTMLInputElement).checked,
                          )
                        }
                        className="ml-2 rounded border-gray-300 dark:border-slate-600 text-blue-600 focus:ring-blue-500 dark:bg-slate-700"
                      />
                    </label>

                <div className="mb-3">
                    <div className="text-sm font-medium text-gray-700 dark:text-gray-200 mb-2">
                      Change State Filter
                    </div>
                    {(["modified", "added", "deleted"] as const).map((s) => {
                      const active = selectorState.split(",").filter(Boolean);
                      const checked = active.includes(s);
                      return (
                        <label key={s} className="flex items-center justify-between mb-1">
                          <span className="text-sm text-gray-700 dark:text-gray-200 capitalize">{s}</span>
                          <input
                            type="checkbox"
                            checked={checked}
                            onChange={() => {
                              const next = checked
                                ? active.filter((x) => x !== s)
                                : [...active, s];
                              setStateFilter(next.join(","));
                            }}
                            className="ml-2 rounded border-gray-300 dark:border-slate-600 text-blue-600 focus:ring-blue-500 dark:bg-slate-700"
                          />
                        </label>
                      );
                    })}
                  </div>

                <div className="mb-3">
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-200 mb-2">
                      Edge Depth Visibility
                    </label>
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-gray-500 dark:text-gray-400">
                        1
                      </span>
                      <input
                        type="range"
                        min="1"
                        max="6"
                        step="1"
                        value={
                          visibleEdgeDepth === Infinity ? 6 : visibleEdgeDepth
                        }
                        onChange={(e) => {
                          const value = parseInt(
                            (e.target as HTMLInputElement).value,
                          );
                          setVisibleEdgeDepth(value === 6 ? Infinity : value);
                        }}
                        className="flex-1 h-2 bg-gray-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer"
                      />
                      <span className="text-xs text-gray-500 dark:text-gray-400">
                        ∞
                      </span>
                    </div>
                    <div className="flex justify-between text-xs text-gray-500 dark:text-gray-400 mt-1">
                      <span>Siblings</span>
                      <span>All edges</span>
                    </div>
                    <div className="text-xs text-gray-600 dark:text-gray-300 mt-1">
                      Current:{" "}
                      {visibleEdgeDepth === Infinity
                        ? "All edges (∞)"
                        : `Depth ${visibleEdgeDepth}`}
                    </div>
                  </div>

                <div className="pt-2 border-t border-gray-100 dark:border-slate-600 space-y-2">
                  <button
                    onClick={onExportJson}
                    className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded bg-white dark:bg-slate-700 border border-gray-200 dark:border-slate-600 hover:bg-gray-50 dark:hover:bg-slate-600"
                  >
                    <IconDownload
                      size={16}
                      strokeWidth={1.5}
                      className="text-gray-600 dark:text-gray-300"
                    />
                    <span className="text-sm text-gray-900 dark:text-gray-100">
                      Export JSON
                    </span>
                  </button>

                  <button
                      onClick={() => {
                        exportPng().catch((error) => {
                          console.error("PNG export failed:", error);
                        });
                      }}
                      className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded bg-white dark:bg-slate-700 border border-gray-200 dark:border-slate-600 hover:bg-gray-50 dark:hover:bg-slate-600"
                    >
                      <IconCamera
                        size={16}
                        strokeWidth={1.5}
                        className="text-gray-600 dark:text-gray-300"
                      />
                      <span className="text-sm text-gray-900 dark:text-gray-100">
                        Download Image
                      </span>
                    </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="mt-2 flex justify-between items-center text-xs text-gray-500 dark:text-gray-400">
        <div>
          Showing {getTotalNodeCount(nodes)} nodes • Supports: wildcards (*),
          upstream (+model), downstream (model+), graph (@model)
        </div>
        <div
          className={
            hasPendingSearch
              ? "text-blue-600 dark:text-blue-400 font-medium"
              : ""
          }
        >
          {hasPendingSearch
            ? "Press Enter or click Search to apply filters"
            : "Type search terms and press Search"}
        </div>
      </div>
    </div>
  );
}
