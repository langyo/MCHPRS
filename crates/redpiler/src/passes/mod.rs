mod clamp_weights;
mod coalesce;
mod constant_coalesce;
mod constant_fold;
mod dedup_links;
mod export_graph;
mod identify_nodes;
mod input_search;
mod prune_orphans;
mod unreachable_output;

use mchprs_world::World;

use super::compile_graph::CompileGraph;
use super::task_monitor::TaskMonitor;
use super::{CompilerInput, CompilerOptions};
use std::sync::Arc;
use std::time::Instant;
use tracing::trace;

pub const fn make_default_pass_manager<'w, W: World>() -> PassManager<'w, W> {
    PassManager::new(&[
        &identify_nodes::IdentifyNodes,
        &input_search::InputSearch,
        &clamp_weights::ClampWeights,
        &dedup_links::DedupLinks,
        &constant_fold::ConstantFold,
        &unreachable_output::UnreachableOutput,
        &constant_coalesce::ConstantCoalesce,
        &coalesce::Coalesce,
        &prune_orphans::PruneOrphans,
        &export_graph::ExportGraph,
    ])
}

pub struct PassManager<'p, W: World> {
    passes: &'p [&'p dyn Pass<W>],
}

impl<'p, W: World> PassManager<'p, W> {
    pub const fn new(passes: &'p [&dyn Pass<W>]) -> Self {
        Self { passes }
    }

    pub fn run_passes(
        &self,
        options: &CompilerOptions,
        input: &CompilerInput<'_, W>,
        monitor: Arc<TaskMonitor>,
    ) -> CompileGraph {
        let mut graph = CompileGraph::new();

        // Add one for the backend compile step
        monitor.set_max_progress(self.passes.len() + 1);

        for &pass in self.passes {
            if !pass.should_run(options) {
                trace!("Skipping pass: {}", pass.name());
                monitor.inc_progress();
                continue;
            }

            if monitor.cancelled() {
                return graph;
            }

            trace!("Running pass: {}", pass.name());
            monitor.set_message(pass.status_message().to_string());
            let start = Instant::now();

            pass.run_pass(&mut graph, options, input);

            trace!("Completed pass in {:?}", start.elapsed());
            trace!("node_count: {}", graph.node_count());
            trace!("edge_count: {}", graph.edge_count());
            monitor.inc_progress();
        }

        graph
    }
}

pub trait Pass<W: World> {
    fn run_pass(
        &self,
        graph: &mut CompileGraph,
        options: &CompilerOptions,
        input: &CompilerInput<'_, W>,
    );

    /// This name should only be use for debugging purposes,
    /// it is not a valid identifier of the pass.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn should_run(&self, options: &CompilerOptions) -> bool {
        // Run passes for optimized builds by default
        options.optimize
    }

    fn status_message(&self) -> &'static str;
}
