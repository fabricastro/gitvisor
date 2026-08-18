//! Lane assignment for the commit DAG.
//!
//! This is the heart of the visual graph. Given commits in topological order
//! (newest first), every commit is placed on a horizontal track — a *lane* —
//! and every commit-to-parent connection becomes an [`Edge`] that already knows
//! which lane it lands in. The renderer therefore never has to walk the DAG
//! itself; it just draws rows.
//!
//! The layout runs in two passes on purpose. The first decides where each
//! commit sits, the second draws the connections. Doing both at once would mean
//! emitting an edge before knowing where its parent finally lands, which is how
//! a long-lived line ends up drifting sideways across the graph.

use std::collections::HashMap;

use crate::model::{Commit, Edge, Graph, GraphRow};

/// Distinct colours cycled through whenever a new line starts.
pub const COLOR_COUNT: u32 = 12;

/// The set of lanes currently in flight during the first pass.
///
/// Each slot holds the id of the commit that lane is still waiting for.
#[derive(Default)]
struct Lanes {
    /// `None` marks a free track. Freed tracks are reused so the graph stays narrow.
    slots: Vec<Option<String>>,
    colors: Vec<u32>,
    next_color: u32,
}

impl Lanes {
    fn find(&self, oid: &str) -> Option<usize> {
        self.slots.iter().position(|s| s.as_deref() == Some(oid))
    }

    fn free(&mut self, lane: usize) {
        self.slots[lane] = None;
    }

    /// Release every track waiting for `oid`, which has just been placed.
    fn release(&mut self, oid: &str) {
        for slot in &mut self.slots {
            if slot.as_deref() == Some(oid) {
                *slot = None;
            }
        }
    }

    /// Park `oid` on a known-free track, keeping the line's colour.
    fn park(&mut self, lane: usize, oid: &str, color: u32) {
        self.slots[lane] = Some(oid.to_string());
        self.colors[lane] = color;
    }

    /// Start a brand new line on the leftmost free track, with a new colour.
    fn start(&mut self, oid: &str) -> usize {
        let lane = match self.slots.iter().position(|s| s.is_none()) {
            Some(lane) => lane,
            None => {
                self.slots.push(None);
                self.colors.push(0);
                self.slots.len() - 1
            }
        };
        let color = self.next_color;
        self.next_color = (self.next_color + 1) % COLOR_COUNT;
        self.park(lane, oid, color);
        lane
    }
}

/// Where every commit ended up after the first pass.
struct Placement {
    lane: Vec<u32>,
    color: Vec<u32>,
    lane_count: u32,
}

/// Lay out `commits` (newest first, topologically ordered) into a drawable graph.
///
/// Runs in O(commits × lanes); lane counts stay small in practice even for
/// repositories with a very long history.
pub fn build(commits: Vec<Commit>, truncated: bool) -> Graph {
    let index_of: HashMap<&str, u32> = commits
        .iter()
        .enumerate()
        .map(|(row, commit)| (commit.id.as_str(), row as u32))
        .collect();

    let placement = place(&commits, &index_of);

    let rows = commits
        .iter()
        .enumerate()
        .map(|(row, commit)| {
            let lane = placement.lane[row];
            let color = placement.color[row];
            let edges = commit
                .parents
                .iter()
                .enumerate()
                .map(|(position, parent)| match index_of.get(parent.as_str()) {
                    Some(&to_row) => Edge {
                        from_lane: lane,
                        to_lane: placement.lane[to_row as usize],
                        to_row: Some(to_row),
                        // The extra parents of a merge belong to the branch
                        // being merged in, so the connector takes its colour.
                        color: if position == 0 {
                            color
                        } else {
                            placement.color[to_row as usize]
                        },
                    },
                    // Parent outside the walked window (truncated history or a
                    // shallow clone): draw a stub running off the bottom.
                    None => Edge {
                        from_lane: lane,
                        to_lane: lane,
                        to_row: None,
                        color,
                    },
                })
                .collect();
            GraphRow {
                commit: commit.clone(),
                lane,
                color,
                edges,
            }
        })
        .collect();

    Graph {
        rows,
        lane_count: placement.lane_count,
        truncated,
    }
}

/// First pass: decide which lane and colour every commit gets.
fn place(commits: &[Commit], index_of: &HashMap<&str, u32>) -> Placement {
    let mut lanes = Lanes::default();
    let mut lane = Vec::with_capacity(commits.len());
    let mut color = Vec::with_capacity(commits.len());
    let mut lane_count = 0u32;

    for commit in commits {
        // A child may already have booked a lane for this commit. If not, this
        // is the tip of a line we have not seen before and gets a fresh colour.
        let current = match lanes.find(&commit.id) {
            Some(existing) => existing,
            None => lanes.start(&commit.id),
        };
        let current_color = lanes.colors[current];

        // The commit occupies the lane on this row; from here on the lane
        // belongs to whichever parent continues the line.
        lanes.release(&commit.id);

        for (position, parent) in commit.parents.iter().enumerate() {
            if !index_of.contains_key(parent.as_str()) {
                // Outside the window: let the line end instead of holding a
                // lane hostage for the rest of the graph.
                continue;
            }
            let reserved = lanes.find(parent);
            if position == 0 {
                match reserved {
                    // An older line further left is already waiting for this
                    // parent. Fold into it so the leftmost line stays straight.
                    Some(other) if other < current => {}
                    // A younger line to the right had claimed it first. Take it
                    // back, otherwise the long-lived line drifts sideways.
                    Some(other) => {
                        lanes.free(other);
                        lanes.park(current, parent, current_color);
                    }
                    None => lanes.park(current, parent, current_color),
                }
            } else if reserved.is_none() {
                // A merged-in branch starts its own line.
                lanes.start(parent);
            }
        }

        lane_count = lane_count.max(current as u32 + 1);
        lane.push(current as u32);
        color.push(current_color);
    }

    Placement {
        lane,
        color,
        lane_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Signature;

    fn commit(id: &str, parents: &[&str]) -> Commit {
        let signature = Signature {
            name: "Test".into(),
            email: "test@example.com".into(),
            timestamp: 0,
            offset_minutes: 0,
        };
        Commit {
            id: id.into(),
            short_id: id.into(),
            summary: id.into(),
            body: String::new(),
            author: signature.clone(),
            committer: signature,
            parents: parents.iter().map(|p| (*p).into()).collect(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn linear_history_stays_on_one_lane() {
        let graph = build(
            vec![commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])],
            false,
        );

        assert_eq!(graph.lane_count, 1);
        assert!(graph.rows.iter().all(|row| row.lane == 0));
        assert!(graph
            .rows
            .iter()
            .all(|row| row.color == graph.rows[0].color));
    }

    #[test]
    fn merge_opens_a_second_lane_and_closes_it_again() {
        // m merges feature (f) into main (a); both descend from the same base.
        let graph = build(
            vec![
                commit("m", &["a", "f"]),
                commit("a", &["base"]),
                commit("f", &["base"]),
                commit("base", &[]),
            ],
            false,
        );

        assert_eq!(graph.lane_count, 2);
        assert_eq!(graph.rows[0].lane, 0, "merge sits on the first-parent lane");
        assert_eq!(graph.rows[1].lane, 0, "first parent inherits the lane");
        assert_eq!(graph.rows[2].lane, 1, "second parent branches off");
        assert_eq!(
            graph.rows[3].lane, 0,
            "shared base returns to the left lane"
        );

        let feature_edge = &graph.rows[0].edges[1];
        assert_eq!(feature_edge.to_lane, 1);
        assert_eq!(feature_edge.to_row, Some(2));
        assert_ne!(
            feature_edge.color, graph.rows[0].color,
            "a branching line gets its own colour"
        );

        // The feature line converges back onto the base lane instead of dying.
        let converging = &graph.rows[2].edges[0];
        assert_eq!(converging.from_lane, 1);
        assert_eq!(converging.to_lane, 0);
        assert_eq!(converging.to_row, Some(3));
    }

    #[test]
    fn freed_lanes_are_reused_so_the_graph_stays_narrow() {
        let graph = build(
            vec![
                commit("t1", &["base"]),
                commit("t2", &["base"]),
                commit("base", &[]),
            ],
            false,
        );

        assert_eq!(graph.lane_count, 2);
        assert_eq!(graph.rows[2].lane, 0);
    }

    #[test]
    fn parent_outside_the_window_becomes_a_stub() {
        let graph = build(vec![commit("a", &["missing"])], true);

        assert!(graph.truncated);
        assert_eq!(graph.rows[0].edges.len(), 1);
        assert_eq!(graph.rows[0].edges[0].to_row, None);
        assert_eq!(graph.lane_count, 1);
    }

    #[test]
    fn the_main_line_never_drifts_when_a_side_branch_claims_its_parent_first() {
        // Regression: `b1` is walked before `merge_a`, and both descend from
        // `m1`. The side branch used to book m1's lane first, dragging the main
        // line off lane 0 for the rest of the graph.
        let graph = build(
            vec![
                commit("merge_b", &["m2", "b1"]),
                commit("m2", &["merge_a"]),
                commit("b1", &["m1"]),
                commit("merge_a", &["m1", "a1"]),
                commit("m1", &["base"]),
                commit("a1", &["base"]),
                commit("base", &[]),
            ],
            false,
        );

        let lane_of = |id: &str| {
            graph
                .rows
                .iter()
                .find(|row| row.commit.id == id)
                .expect("commit in graph")
                .lane
        };

        for id in ["merge_b", "m2", "merge_a", "m1", "base"] {
            assert_eq!(lane_of(id), 0, "{id} belongs to the straight main line");
        }
        assert_eq!(lane_of("b1"), 1);
        assert_eq!(lane_of("a1"), 1, "the freed side lane is reused");
        assert_eq!(graph.lane_count, 2);

        // Every edge points at the lane its parent is really drawn in.
        for row in &graph.rows {
            for edge in &row.edges {
                if let Some(to_row) = edge.to_row {
                    assert_eq!(edge.to_lane, graph.rows[to_row as usize].lane);
                }
            }
        }
    }
}
