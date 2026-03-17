export type NodeClasses = {
  container: string;
  containerSelected: string;
  containerHighlighted: string;
  containerDimmed: string;
  containerHover: string;
  header: string;
  headerHover: string;
  headerSelected: string;
  headerHighlighted: string;
  headerDimmed: string;
  toggleButton: string;
  toggleText: string;
  titleText: string;
  loadingStroke: string;
  loadingText: string;
  leaf: string;
  leafSelected: string;
  leafHighlighted: string;
  leafDimmed: string;
  leafText: string;
};

const NODE_CLASSES: NodeClasses = {
  container:
    "fill-emerald-50 dark:fill-slate-800 stroke-emerald-500 dark:stroke-emerald-400 transition-colors duration-200 hover:fill-emerald-100 hover:stroke-emerald-600 dark:hover:fill-slate-700",
  containerSelected:
    "fill-emerald-100 dark:fill-slate-700 stroke-emerald-400 dark:stroke-emerald-300",
  containerHighlighted:
    "fill-emerald-100 dark:fill-slate-800 stroke-emerald-600 dark:stroke-emerald-400",
  containerDimmed:
    "fill-teal-100 dark:fill-slate-900 stroke-teal-200 dark:stroke-teal-700",
  containerHover:
    "fill-emerald-100 dark:fill-slate-900 stroke-emerald-600 dark:stroke-emerald-200",
  header:
    "fill-emerald-600 dark:fill-emerald-700 transition-colors duration-200",
  headerHover: "fill-emerald-500 dark:fill-slate-800",
  headerSelected: "fill-emerald-500 dark:fill-emerald-300",
  headerHighlighted: "fill-emerald-700 dark:fill-emerald-600",
  headerDimmed: "fill-teal-200 dark:fill-teal-900",
  toggleButton:
    "fill-white dark:fill-gray-800 stroke-emerald-600 dark:stroke-emerald-400",
  toggleText: "fill-emerald-600 dark:fill-emerald-400",
  titleText: "fill-gray-900 dark:fill-gray-100",
  loadingStroke: "stroke-emerald-600 dark:stroke-emerald-400",
  loadingText: "fill-emerald-600 dark:fill-emerald-400",
  leaf: "fill-emerald-200 dark:fill-emerald-800 stroke-emerald-800 dark:stroke-emerald-500",
  leafSelected: "fill-emerald-300 dark:fill-emerald-700",
  leafHighlighted: "fill-emerald-400 dark:fill-emerald-600",
  leafDimmed: "fill-teal-300 dark:fill-teal-800",
  leafText: "fill-gray-800 dark:fill-gray-200",
};

export const getNodeClasses = (): NodeClasses => NODE_CLASSES;
