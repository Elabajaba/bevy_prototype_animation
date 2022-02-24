mod node;

use node::*;

use crate::AnimationClip;
use bevy_asset::Handle;
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
struct ClipState {
    weight: f32,
    time: f32,
}

#[derive(Default, Debug)]
pub(self) struct GraphState {
    clips: Vec<ClipState>,
}

impl GraphState {
    /// Gets the number of clips that in the graph.
    pub fn clip_count(&self) -> u16 {
        self.clips.len() as u16
    }

    /// Creates a new state for a clip. Returns the corresponding
    /// internal ID for the clip.
    pub fn add_clip(&mut self) -> ClipId {
        assert!(self.clip_weights.len() < u16::MAX as usize);
        let clip_id = ClipId(self.clip_weights.len() as u16);
        self.clips.push(Default::default());
        clip_id
    }

    /// Sets the time for a given clip in the current state of the
    /// graph.
    ///
    /// # Panics
    /// This will panic if `clip` isn't a valid `ClipId`.
    pub fn set_time(&mut self, clip: ClipId, time: f32) {
        self.clip_times[clip.0 as usize].time = time;
    }

    /// Advances time by a specific delta for all clips in the
    /// graph.
    pub fn advance_time(&mut self, delta_time: f32) {
        for clip in self.clips.iter_mut() {
            *clip.time += delta_time;
        }
    }

    /// Resets weights for all clips in the graph to 0.
    pub fn clear_weights(&mut self) {
        for influence in self.clip_influences.iter_mut() {
            *clip.influence = 0.0;
        }
    }

    /// Adds a change in weights to a specific clip in the current
    /// state in the graph.
    ///
    /// # Panics
    /// This will panic if `clip` isn't a valid `ClipId`.
    pub fn add_weight(&mut self, clip: ClipId, delta_weight: f32) {
        self.clips[clip.0 as usize].weight += delta_weight;
    }
}

/// A temporary state for tracking visited but unexplored nodes in
/// the graph during evaluation.
struct GraphTraversalNode {
    node_id: NodeId,
    cumulative_weight: f32,
}

pub enum AnimationGraphError {
    NodeNotFound(NodeId),
    InputAlreadyExists(NodeId),
}

pub struct AnimationGraph {
    nodes: GraphNodes,
    state: GraphState,
}

impl AnimationGraph {
    pub fn add_input(
        &mut self,
        target: NodeId,
        input: NodeId,
    ) -> Result<&mut NodeInput, AnimationGraphError> {
        // TODO: Check for cycles before adding edge.

        self.nodes
            .get(input)
            .ok_or(AnimationGraphError::NodeNotFound(input))?;

        let target = self
            .nodes
            .get_mut(target)
            .ok_or(AnimationGraphError::NodeNotFound(target))?;

        if target.get_input_mut(input).is_some() {
            Err(AnimationGraphError::InputAlreadyExists(input))
        } else {
            target.inputs.push(NodeInput::new(input));
            Ok(target.inputs.last_mut().unwrap())
        }
    }

    /// Adds an [`AnimationClip`] as a node in the graph.
    ///
    /// Returns the corresponding node ID.
    pub fn add_clip(&mut self, clip: &AnimationClip) -> NodeId {
        let clip_id = self.state.add_clip();
        // TODO: Copy curves from the provided animation clip into curve
        // storage.
        self.nodes.add(Node::create_leaf(clip))
    }

    /// Sets the time for a given node. If the node is set to propagate its
    /// time, all of it's currently connected inputs will also have the time
    /// propagated to them as well.
    pub fn set_time(&mut self, node_id: NodeId, time: f32) -> Result<(), AnimationGraphError> {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.local_time = time;
            if !node.propogate_time {
                return Ok(());
            }
        } else {
            return Err(AnimationGraphError::NodeNotFound(node_id));
        }

        //
        // TODO: Cache this to avoid allocations in the future.
        let mut pending = VecDeque::new();
        pending.push_back(node_id);
        while let Some(node_id) = pending.pop_front() {
            if let Some(node) = self.nodes.get(node_id) {
                let node = if let Some(node) = self.nodes.get_mut(node_id) {
                    node.local_time = time;
                    node
                } else {
                    continue;
                };
                if node.propogate_time {
                    pending.extend(node.connected_inputs().map(|input| input.node_id()));
                }
            }
        }

        Ok(())
    }

    /// Evaluates the graph, computing the influences individual results.
    pub fn evaluate(&mut self) {
        self.state.reset();

        // TODO: Use smallvec to avoid allocation here.
        let stack = vec![GraphTraversalNode {
            node_id: NodeId::ROOT,
            cumulative_weight: 1.0,
        }];

        // Conduct a depth-first traversal of the graph multiplying the weights
        // as it gets deeper into the tree.
        while let Some(current) = stack.pop() {
            let current_node = if let Some(node) = self.nodes.get(current.node_id) {
                node
            } else {
                continue;
            };

            if let Some(clip) = current.clip {
                self.state.add_influence(clip, current_node.weight);
            }

            for input in self.connected_inputs() {
                let cumulative_weight = input.weight * current_node.cumulative_weight;
                if cumulative_weight != 0.0 {
                    stack.push(GraphTraversalNode {
                        node_id: input.node_id,
                        cumulative_weight,
                    });
                }
            }
        }
    }
}
