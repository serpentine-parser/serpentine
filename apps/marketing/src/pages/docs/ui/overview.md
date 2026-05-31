---
layout: ../../../layouts/Docs.astro
title: UI Overview
description: The five zones of the Serpentine graph viewer.
---

When you run `serpentine serve`, Serpentine opens a browser window with an interactive graph viewer. The interface is divided into five zones that work together.

![Serpentine UI overview with all five zones visible](/docs/ui/UI_Overview.png)

---

## 1. Header

The top bar runs the full width of the screen. It contains the Serpentine logo (which links to the homepage) and a settings button on the right. Click the settings button to open a dropdown with layout controls, spacing adjustments, theme switching, and expand/collapse-all shortcuts. See [Reference → Header](/docs/ui/reference#header) for every option.

## 2. Object Explorer

The collapsible sidebar on the left lists every node in the graph as a tree: modules at the top level, with classes and functions nested inside. You can search the tree, expand or collapse branches, and click any node to select it and jump to it in the canvas. When files change on disk, the sidebar highlights added, modified, and deleted nodes in color. See [Reference → Object Explorer](/docs/ui/reference#object-explorer) for details.

## 3. Detail Panel

When a node is selected the Detail Panel opens on the right side of the canvas. It shows the node's full ID, type, source location, docstring, syntax-highlighted source code, and two lists: the node's dependencies and the nodes that depend on it. Every entry in those lists is clickable. See [Reference → Detail Panel](/docs/ui/reference#detail-panel).

## 4. Graph Canvas

The central area is an SVG canvas showing the code reference graph. You can pan by dragging and zoom with the scroll wheel. Each rectangle is a node (module, class, function, assignment, etc); the arrows between them are edges that encode the relationship type. Clicking a node selects it, opens the Detail Panel, and reveals a mini-toolbar for expanding or collapsing its children. See [Reference → Graph Canvas](/docs/ui/reference#graph-canvas).

## 5. Search Bar

The floating bar at the bottom of the canvas is the primary way to filter the graph. Type a selector expression in the include field (and optionally an exclude expression in the second field), then press Enter or click Search. The graph updates to show only the matching subgraph. A settings gear opens additional options: package filters, edge depth control, change state filters, and export actions. See [Reference → Search Bar](/docs/ui/reference#search-bar).

---

## Next steps

- [UI Reference](/docs/ui/reference) — every control documented
- [Selectors](/docs/querying/selectors) — the selector syntax used in the search bar
