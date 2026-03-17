import { GraphState, Node } from '../../graph/model/types';

export class SearchAndFilter {
  filterNodesByOrigin(nodeList: Node[], state: GraphState): Node[] {
    return nodeList.filter((node) => {
      let origin = node.origin;

      if (!origin) {
        if (node.id.startsWith("src")) {
          origin = "local";
        } else {
          const stdlibModules = new Set([
            "builtins", "sys", "os", "json", "pathlib", "re", "math", "random",
            "datetime", "time", "collections", "itertools", "functools", "operator",
            "string", "io", "codecs", "pickle", "sqlite3", "logging", "unittest",
            "abc", "contextlib", "inspect", "typing", "dataclasses",
          ]);
          origin = stdlibModules.has(node.id.split(".")[0]) ? "standard" : "third-party";
        }
      }

      if (origin === "standard") return state.includeStandardPackages;
      if (origin === "third-party") return state.includeThirdPartyPackages;
      return true;
    });
  }

  filterNodes(state: GraphState): { nodes: Node[] } {
    const originFilteredNodes = this.filterNodesByOrigin(state.allNodes, state);
    return { nodes: originFilteredNodes };
  }
}
