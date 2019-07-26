use super::*;
use floating_time_dependent::*;
use crate::report::*;
use std::{
    cmp::min,
    sync::atomic::Ordering,
};

pub fn customize<'a, 'b: 'a>(cch: &'a CCH, metric: &'b TDGraph) -> ShortcutGraph<'a> {
    report!("algo", "Floating TDCCH Customization");

    let n = (cch.first_out.len() - 1) as NodeId;
    let m = cch.head.len();

    let mut upward: Vec<_> = std::iter::repeat_with(|| Shortcut::new(None, metric)).take(m).collect();
    let mut downward: Vec<_> = std::iter::repeat_with(|| Shortcut::new(None, metric)).take(m).collect();

    let subctxt = push_context("weight_applying".to_string());
    report_time("TD-CCH apply weights", || {
        upward.par_iter_mut().zip(downward.par_iter_mut()).zip(cch.cch_edge_to_orig_arc.par_iter()).for_each(|((up_weight, down_weight), &(up_arc, down_arc))| {
            if let Some(up_arc) = up_arc.value() {
                *up_weight = Shortcut::new(Some(up_arc), metric);
            }
            if let Some(down_arc) = down_arc.value() {
                *down_weight = Shortcut::new(Some(down_arc), metric);
            }
        });
    });
    drop(subctxt);

    if cfg!(feature = "tdcch-precustomization") {
        let _subctxt = push_context("precustomization".to_string());
        report_time("TD-CCH Pre-Customization", || {
            let mut node_edge_ids = vec![InRangeOption::new(None); n as usize];

            for current_node in 0..n {
                for (node, edge_id) in cch.neighbor_iter(current_node).zip(cch.neighbor_edge_indices(current_node)) {
                    upward[edge_id as usize].update_is_constant();
                    downward[edge_id as usize].update_is_constant();
                    node_edge_ids[node as usize] = InRangeOption::new(Some(edge_id));
                }

                for (node, edge_id) in cch.neighbor_iter(current_node).zip(cch.neighbor_edge_indices(current_node)) {
                    debug_assert_eq!(cch.edge_id_to_tail(edge_id), current_node);
                    let shortcut_edge_ids = cch.neighbor_edge_indices(node);
                    for (target, shortcut_edge_id) in cch.neighbor_iter(node).zip(shortcut_edge_ids) {
                        debug_assert_eq!(cch.edge_id_to_tail(shortcut_edge_id), node);
                        if let Some(other_edge_id) = node_edge_ids[target as usize].value() {
                            debug_assert!(shortcut_edge_id > edge_id);
                            debug_assert!(shortcut_edge_id > other_edge_id);
                            upward[shortcut_edge_id as usize].upper_bound = min(upward[shortcut_edge_id as usize].upper_bound, downward[edge_id as usize].upper_bound + upward[other_edge_id as usize].upper_bound);
                            upward[shortcut_edge_id as usize].lower_bound = min(upward[shortcut_edge_id as usize].lower_bound, downward[edge_id as usize].lower_bound + upward[other_edge_id as usize].lower_bound);
                            downward[shortcut_edge_id as usize].upper_bound = min(downward[shortcut_edge_id as usize].upper_bound, downward[other_edge_id as usize].upper_bound + upward[edge_id as usize].upper_bound);
                            downward[shortcut_edge_id as usize].lower_bound = min(downward[shortcut_edge_id as usize].lower_bound, downward[other_edge_id as usize].lower_bound + upward[edge_id as usize].lower_bound);
                        }
                    }
                }

                for node in cch.neighbor_iter(current_node) {
                    node_edge_ids[node as usize] = InRangeOption::new(None);
                }
            }

            let upward_preliminary_bounds: Vec<_> = upward.iter().map(|s| s.lower_bound).collect();
            let downward_preliminary_bounds: Vec<_> = downward.iter().map(|s| s.lower_bound).collect();

            for current_node in (0..n).rev() {
                for (node, edge_id) in cch.neighbor_iter(current_node).zip(cch.neighbor_edge_indices(current_node)) {
                    node_edge_ids[node as usize] = InRangeOption::new(Some(edge_id));
                }

                for (node, edge_id) in cch.neighbor_iter(current_node).zip(cch.neighbor_edge_indices(current_node)) {
                    let shortcut_edge_ids = cch.neighbor_edge_indices(node);
                    for (target, shortcut_edge_id) in cch.neighbor_iter(node).zip(shortcut_edge_ids) {
                        if let Some(other_edge_id) = node_edge_ids[target as usize].value() {
                            upward[other_edge_id as usize].upper_bound = min(upward[other_edge_id as usize].upper_bound, upward[edge_id as usize].upper_bound + upward[shortcut_edge_id as usize].upper_bound);
                            upward[other_edge_id as usize].lower_bound = min(upward[other_edge_id as usize].lower_bound, upward[edge_id as usize].lower_bound + upward[shortcut_edge_id as usize].lower_bound);

                            upward[edge_id as usize].upper_bound = min(upward[edge_id as usize].upper_bound, upward[other_edge_id as usize].upper_bound + downward[shortcut_edge_id as usize].upper_bound);
                            upward[edge_id as usize].lower_bound = min(upward[edge_id as usize].lower_bound, upward[other_edge_id as usize].lower_bound + downward[shortcut_edge_id as usize].lower_bound);

                            downward[other_edge_id as usize].upper_bound = min(downward[other_edge_id as usize].upper_bound, downward[edge_id as usize].upper_bound + downward[shortcut_edge_id as usize].upper_bound);
                            downward[other_edge_id as usize].lower_bound = min(downward[other_edge_id as usize].lower_bound, downward[edge_id as usize].lower_bound + downward[shortcut_edge_id as usize].lower_bound);

                            downward[edge_id as usize].upper_bound = min(downward[edge_id as usize].upper_bound, downward[other_edge_id as usize].upper_bound + upward[shortcut_edge_id as usize].upper_bound);
                            downward[edge_id as usize].lower_bound = min(downward[edge_id as usize].lower_bound, downward[other_edge_id as usize].lower_bound + upward[shortcut_edge_id as usize].lower_bound);
                        }
                    }
                }

                for node in cch.neighbor_iter(current_node) {
                    node_edge_ids[node as usize] = InRangeOption::new(None);
                }
            }

            for (shortcut, lower_bound) in upward.iter_mut().zip(upward_preliminary_bounds.into_iter()) {
                if shortcut.upper_bound.fuzzy_lt(lower_bound) {
                    shortcut.required = false;
                    shortcut.lower_bound = FlWeight::INFINITY;
                    shortcut.upper_bound = FlWeight::INFINITY;
                } else {
                    shortcut.lower_bound = lower_bound;
                }
            }

            for (shortcut, lower_bound) in downward.iter_mut().zip(downward_preliminary_bounds.into_iter()) {
                if shortcut.upper_bound.fuzzy_lt(lower_bound) {
                    shortcut.required = false;
                    shortcut.lower_bound = FlWeight::INFINITY;
                    shortcut.upper_bound = FlWeight::INFINITY;
                } else {
                    shortcut.lower_bound = lower_bound;
                }
            }
        });
    }

    {
        let subctxt = push_context("main".to_string());

        use std::thread;
        use std::sync::mpsc::{channel, RecvTimeoutError};

        let (tx, rx) = channel();
        let (events_tx, events_rx) = channel();

        let customization = SeperatorBasedParallelCustomization::new(cch, create_customization_fn(&cch, metric, SeqIter(&cch)), create_customization_fn(&cch, metric, ParIter(&cch)));

        report_time("TD-CCH Customization", || {
            thread::spawn(move || {
                let timer = Timer::new();

                let mut events = Vec::new();

                loop {
                    report!("at_s", timer.get_passed_ms() / 1000);
                    report!("nodes_customized", NODES_CUSTOMIZED.load(Ordering::Relaxed));
                    if cfg!(feature = "detailed-stats") {
                        report!("num_ipps_stored", IPP_COUNT.load(Ordering::Relaxed));
                        report!("num_shortcuts_active", ACTIVE_SHORTCUTS.load(Ordering::Relaxed));
                        report!("num_ipps_reduced_by_approx", SAVED_BY_APPROX.load(Ordering::Relaxed));
                        report!("num_ipps_considered_for_approx", CONSIDERED_FOR_APPROX.load(Ordering::Relaxed));
                        report!("num_shortcut_merge_points", PATH_SOURCES_COUNT.load(Ordering::Relaxed));
                        report!("num_performed_merges", ACTUALLY_MERGED.load(Ordering::Relaxed));
                        report!("num_performed_links", ACTUALLY_LINKED.load(Ordering::Relaxed));
                        report!("num_performed_unnecessary_links", UNNECESSARY_LINKED.load(Ordering::Relaxed));
                    }

                    if cfg!(feature = "detailed-stats") {
                        events.push((timer.get_passed_ms() / 1000,
                                     NODES_CUSTOMIZED.load(Ordering::Relaxed),
                                     IPP_COUNT.load(Ordering::Relaxed),
                                     ACTIVE_SHORTCUTS.load(Ordering::Relaxed),
                                     SAVED_BY_APPROX.load(Ordering::Relaxed),
                                     CONSIDERED_FOR_APPROX.load(Ordering::Relaxed),
                                     PATH_SOURCES_COUNT.load(Ordering::Relaxed),
                                     ACTUALLY_MERGED.load(Ordering::Relaxed),
                                     ACTUALLY_LINKED.load(Ordering::Relaxed),
                                     UNNECESSARY_LINKED.load(Ordering::Relaxed)));
                    } else {
                        events.push((timer.get_passed_ms() / 1000, NODES_CUSTOMIZED.load(Ordering::Relaxed), 0, 0, 0, 0, 0, 0, 0, 0));
                    }


                    if let Ok(()) | Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(std::time::Duration::from_secs(3)) {
                        events_tx.send(events).unwrap();
                        break;
                    }
                }
            });

            customization.customize(&mut upward, &mut downward);
        });

        tx.send(()).unwrap();

        for events in events_rx {
            let mut events_ctxt = push_collection_context("events".to_string());

            for event in events {
                let _event = events_ctxt.push_collection_item();

                report_silent!("at_s", event.0);
                report_silent!("nodes_customized", event.1);
                if cfg!(feature = "detailed-stats") {
                    report_silent!("num_ipps_stored", event.2);
                    report_silent!("num_shortcuts_active", event.3);
                    report_silent!("num_ipps_reduced_by_approx", event.4);
                    report_silent!("num_ipps_considered_for_approx", event.5);
                    report_silent!("num_shortcut_merge_points", event.6);
                    report_silent!("num_performed_merges", event.7);
                    report_silent!("num_performed_links", event.8);
                    report_silent!("num_performed_unnecessary_links", event.9);
                }
            }
        }

        drop(subctxt);
    }

    if cfg!(feature = "detailed-stats") {
        report!("num_ipps_stored", IPP_COUNT.load(Ordering::Relaxed));
        report!("num_shortcuts_active", ACTIVE_SHORTCUTS.load(Ordering::Relaxed));
        report!("num_ipps_reduced_by_approx", SAVED_BY_APPROX.load(Ordering::Relaxed));
        report!("num_ipps_considered_for_approx", CONSIDERED_FOR_APPROX.load(Ordering::Relaxed));
        report!("num_shortcut_merge_points", PATH_SOURCES_COUNT.load(Ordering::Relaxed));
        report!("num_performed_merges", ACTUALLY_MERGED.load(Ordering::Relaxed));
        report!("num_performed_links", ACTUALLY_LINKED.load(Ordering::Relaxed));
        report!("num_performed_unnecessary_links", UNNECESSARY_LINKED.load(Ordering::Relaxed));
    }
    report!("approx", f64::from(APPROX));
    report!("approx_threshold", APPROX_THRESHOLD);

    if cfg!(feature = "tdcch-postcustomization") {
        let _subctxt = push_context("postcustomization".to_string());
        report_time("TD-CCH Post-Customization", || {
            let mut removed_by_perfection = 0;
            let mut node_edge_ids = vec![InRangeOption::new(None); n as usize];

            let upward_preliminary_bounds: Vec<_> = upward.iter().map(|s| s.lower_bound).collect();
            let downward_preliminary_bounds: Vec<_> = downward.iter().map(|s| s.lower_bound).collect();

            for current_node in (0..n).rev() {
                for (node, edge_id) in cch.neighbor_iter(current_node).zip(cch.neighbor_edge_indices(current_node)) {
                    node_edge_ids[node as usize] = InRangeOption::new(Some(edge_id));
                }

                for (node, edge_id) in cch.neighbor_iter(current_node).zip(cch.neighbor_edge_indices(current_node)) {
                    let shortcut_edge_ids = cch.neighbor_edge_indices(node);
                    for (target, shortcut_edge_id) in cch.neighbor_iter(node).zip(shortcut_edge_ids) {
                        if let Some(other_edge_id) = node_edge_ids[target as usize].value() {
                            upward[other_edge_id as usize].upper_bound = min(upward[other_edge_id as usize].upper_bound, upward[edge_id as usize].upper_bound + upward[shortcut_edge_id as usize].upper_bound);
                            upward[other_edge_id as usize].lower_bound = min(upward[other_edge_id as usize].lower_bound, upward[edge_id as usize].lower_bound + upward[shortcut_edge_id as usize].lower_bound);

                            upward[edge_id as usize].upper_bound = min(upward[edge_id as usize].upper_bound, upward[other_edge_id as usize].upper_bound + downward[shortcut_edge_id as usize].upper_bound);
                            upward[edge_id as usize].lower_bound = min(upward[edge_id as usize].lower_bound, upward[other_edge_id as usize].lower_bound + downward[shortcut_edge_id as usize].lower_bound);

                            downward[other_edge_id as usize].upper_bound = min(downward[other_edge_id as usize].upper_bound, downward[edge_id as usize].upper_bound + downward[shortcut_edge_id as usize].upper_bound);
                            downward[other_edge_id as usize].lower_bound = min(downward[other_edge_id as usize].lower_bound, downward[edge_id as usize].lower_bound + downward[shortcut_edge_id as usize].lower_bound);

                            downward[edge_id as usize].upper_bound = min(downward[edge_id as usize].upper_bound, downward[other_edge_id as usize].upper_bound + upward[shortcut_edge_id as usize].upper_bound);
                            downward[edge_id as usize].lower_bound = min(downward[edge_id as usize].lower_bound, downward[other_edge_id as usize].lower_bound + upward[shortcut_edge_id as usize].lower_bound);
                        }
                    }
                }

                for node in cch.neighbor_iter(current_node) {
                    node_edge_ids[node as usize] = InRangeOption::new(None);
                }
            }

            for (shortcut, lower_bound) in upward.iter_mut().zip(upward_preliminary_bounds.into_iter()) {
                if shortcut.upper_bound.fuzzy_lt(lower_bound) {
                    if shortcut.required { removed_by_perfection += 1; }
                    shortcut.required = false;
                    shortcut.lower_bound = FlWeight::INFINITY;
                    shortcut.upper_bound = FlWeight::INFINITY;
                } else {
                    shortcut.lower_bound = lower_bound;
                }
            }

            for (shortcut, lower_bound) in downward.iter_mut().zip(downward_preliminary_bounds.into_iter()) {
                if shortcut.upper_bound.fuzzy_lt(lower_bound) {
                    if shortcut.required { removed_by_perfection += 1; }
                    shortcut.required = false;
                    shortcut.lower_bound = FlWeight::INFINITY;
                    shortcut.upper_bound = FlWeight::INFINITY;
                } else {
                    shortcut.lower_bound = lower_bound;
                }
            }

            for current_node in 0..n {
                let (upward_below, upward_above) = upward.split_at_mut(cch.first_out[current_node as usize] as usize);
                let upward_active = &mut upward_above[0..cch.neighbor_edge_indices(current_node as NodeId).len()];
                let (downward_below, downward_above) = downward.split_at_mut(cch.first_out[current_node as usize] as usize);
                let downward_active = &mut downward_above[0..cch.neighbor_edge_indices(current_node as NodeId).len()];
                let shortcut_graph = PartialShortcutGraph::new(metric, upward_below, downward_below, 0);

                for shortcut in &mut upward_active[..] {
                    shortcut.invalidate_unneccesary_sources(&shortcut_graph);
                }

                for shortcut in &mut downward_active[..] {
                    shortcut.invalidate_unneccesary_sources(&shortcut_graph);
                }
            }

            report!("removed_by_perfection", removed_by_perfection);
        });
    }

    ShortcutGraph::new(metric, &cch.first_out, &cch.head, upward, downward)
}

fn create_customization_fn<'s, F: 's>(cch: &'s CCH, metric: &'s TDGraph, merge_iter: F) -> impl Fn(Range<usize>, usize, &mut [Shortcut], &mut [Shortcut]) + 's where
    for <'p> F: ForEachIter<'p, 's>,
{
    move |nodes, edge_offset, upward: &mut [Shortcut], downward: &mut [Shortcut]| {
        for current_node in nodes {

            let (upward_below, upward_above) = upward.split_at_mut(cch.first_out[current_node as usize] as usize - edge_offset);
            let upward_active = &mut upward_above[0..cch.neighbor_edge_indices(current_node as NodeId).len()];
            let (downward_below, downward_above) = downward.split_at_mut(cch.first_out[current_node as usize] as usize - edge_offset);
            let downward_active = &mut downward_above[0..cch.neighbor_edge_indices(current_node as NodeId).len()];
            let shortcut_graph = PartialShortcutGraph::new(metric, upward_below, downward_below, edge_offset);

            debug_assert_eq!(upward_active.len(), cch.degree(current_node as NodeId));
            debug_assert_eq!(downward_active.len(), cch.degree(current_node as NodeId));

            merge_iter.for_each(current_node as NodeId, upward_active, downward_active, |((&node, upward_shortcut), downward_shortcut)| {
                MERGE_BUFFERS.with(|buffers| {
                    let mut buffers = buffers.borrow_mut();

                    let mut triangles = Vec::new();

                    let mut current_iter = cch.inverted.neighbor_iter(current_node as NodeId).peekable();
                    let mut other_iter = cch.inverted.neighbor_iter(node as NodeId).peekable();

                    while let (Some(Link { node: lower_from_current, weight: edge_from_cur_id }), Some(Link { node: lower_from_other, weight: edge_from_oth_id })) = (current_iter.peek(), other_iter.peek()) {
                        debug_assert_eq!(cch.head()[*edge_from_cur_id as usize], current_node as NodeId);
                        debug_assert_eq!(cch.head()[*edge_from_oth_id as usize], node);
                        debug_assert_eq!(cch.edge_id_to_tail(*edge_from_cur_id), *lower_from_current);
                        debug_assert_eq!(cch.edge_id_to_tail(*edge_from_oth_id), *lower_from_other);

                        if lower_from_current < lower_from_other {
                            current_iter.next();
                        } else if lower_from_other < lower_from_current {
                            other_iter.next();
                        } else {
                            triangles.push((*edge_from_cur_id, *edge_from_oth_id));

                            current_iter.next();
                            other_iter.next();
                        }
                    }
                    if cfg!(feature = "tdcch-triangle-sorting") {
                        triangles.sort_by_key(|&(down, up)| shortcut_graph.get_incoming(down).lower_bound + shortcut_graph.get_outgoing(up).lower_bound);
                    }
                    for &edges in &triangles {
                        upward_shortcut.merge(edges, &shortcut_graph, &mut buffers);
                    }
                    upward_shortcut.finalize_bounds(&shortcut_graph);

                    if cfg!(feature = "tdcch-triangle-sorting") {
                        triangles.sort_by_key(|&(up, down)| shortcut_graph.get_incoming(down).lower_bound + shortcut_graph.get_outgoing(up).lower_bound);
                    }
                    for &(up, down) in &triangles {
                        downward_shortcut.merge((down, up), &shortcut_graph, &mut buffers);
                    }
                    downward_shortcut.finalize_bounds(&shortcut_graph);
                });
            });

            for Link { weight: edge_id, .. } in cch.inverted.neighbor_iter(current_node as NodeId) {
                upward[edge_id as usize - edge_offset].clear_plf();
                downward[edge_id as usize - edge_offset].clear_plf();
            }

            NODES_CUSTOMIZED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

thread_local! { static MERGE_BUFFERS: RefCell<MergeBuffers> = RefCell::new(MergeBuffers::new()); }

trait ForEachIter<'s, 'c> {
    fn for_each(&self, current_node: NodeId, upward_active: &'s mut [Shortcut], downward_active: &'s mut [Shortcut], f: impl Send + Sync + Fn(((&'c NodeId, &'s mut Shortcut), &'s mut Shortcut)));
}

struct SeqIter<'c>(&'c CCH);

impl<'s, 'c> ForEachIter<'s, 'c> for SeqIter<'c> {
    fn for_each(&self, current_node: NodeId, upward_active: &'s mut [Shortcut], downward_active: &'s mut [Shortcut], f: impl Send + Sync + Fn(((&'c NodeId, &'s mut Shortcut), &'s mut Shortcut))) {
        self.0.head[self.0.neighbor_edge_indices_usize(current_node)].iter()
            .zip(upward_active.iter_mut())
            .zip(downward_active.iter_mut())
            .for_each(f);
    }
}

struct ParIter<'c>(&'c CCH);

impl<'s, 'c> ForEachIter<'s, 'c> for ParIter<'c> {
    fn for_each(&self, current_node: NodeId, upward_active: &'s mut [Shortcut], downward_active: &'s mut [Shortcut], f: impl Send + Sync + Fn(((&'c NodeId, &'s mut Shortcut), &'s mut Shortcut))) {
        self.0.head[self.0.neighbor_edge_indices_usize(current_node)].par_iter()
            .zip_eq(upward_active.par_iter_mut())
            .zip_eq(downward_active.par_iter_mut())
            .for_each(f);
    }
}