/**
 * Lane palette. Twelve hues that stay distinguishable next to each other on a
 * dark background; the Rust side cycles through the same count.
 */
const LANE_COLORS = [
  "#4c9aff",
  "#f778ba",
  "#7ee787",
  "#f0883e",
  "#a371f7",
  "#39d3c3",
  "#e3b341",
  "#ff7b72",
  "#79c0ff",
  "#d2a8ff",
  "#56d364",
  "#ffa657",
] as const;

export const laneColor = (index: number) => LANE_COLORS[index % LANE_COLORS.length];

/** Fill used to punch merge nodes hollow; must match the row background. */
export const GRAPH_BACKGROUND = "#0d1117";
