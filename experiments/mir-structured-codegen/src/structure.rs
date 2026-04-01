//! MIR control-flow structuring.
//!
//! Converts a flat basic-block CFG into a structured C-like AST
//! (if/else, while, switch) without goto statements where possible.
//!
//! The algorithm exploits the fact that Rust MIR always produces reducible
//! CFGs, so every loop has a single entry point (natural loop) and control
//! flow can be decomposed into nested single-entry regions.
//!
//! ## Approach
//!
//! 1. **Pre-pass**: DFS to discover back-edges (loop headers).
//! 2. **Recursive descent**: starting from bb0, translate each block:
//!    - `Goto` -> continue to the target in the same sequence.
//!    - `Return` -> emit return statement.
//!    - `Drop` -> emit drop + continue.
//!    - `Call` -> emit call + continue.
//!    - `SwitchInt` (2 targets) -> `if/else`, finding the merge point
//!      where both branches reconverge.
//!    - `SwitchInt` (N targets) -> `switch`.
//!    - When reaching a loop header, emit `while` with the loop body
//!      structured recursively.

use std::collections::{HashMap, HashSet, VecDeque};

use super::types::{CExpr, CStmt, MirBlock, MirFunc, MirStmt, MirTerminator};

/// Structurize a simplified MIR function into a list of [`CStmt`].
pub fn structurize(func: &MirFunc) -> Vec<CStmt> {
    let ctx = StructCtx::new(func);
    ctx.structure_func()
}

struct StructCtx<'a> {
    func: &'a MirFunc,
    /// Set of block indices that are loop headers (targets of back-edges).
    loop_headers: HashSet<usize>,
    /// For each loop header, the set of blocks in the loop body.
    loop_bodies: HashMap<usize, HashSet<usize>>,
    /// For each loop header, the exit block (first block outside the loop
    /// that the loop branches to).
    loop_exits: HashMap<usize, usize>,
    /// Successors adjacency list.
    successors: Vec<Vec<usize>>,
}

impl<'a> StructCtx<'a> {
    fn new(func: &'a MirFunc) -> Self {
        let n = func.blocks.len();
        let mut successors = vec![Vec::new(); n];
        for blk in &func.blocks {
            if !blk.is_cleanup {
                for s in term_successors(&blk.terminator) {
                    if s < n && !func.blocks[s].is_cleanup {
                        successors[blk.index].push(s);
                    }
                }
            }
        }

        let (loop_headers, back_edges) = find_back_edges(n, &successors);
        let loop_bodies = compute_loop_bodies(&back_edges, &successors, n);
        let loop_exits = compute_loop_exits(&loop_bodies, &successors);

        Self {
            func,
            loop_headers,
            loop_bodies,
            loop_exits,
            successors,
        }
    }

    fn structure_func(&self) -> Vec<CStmt> {
        let mut visited = HashSet::new();
        self.structure_from(0, &mut visited, None)
    }

    /// Recursively structure blocks starting at `bb`, stopping if we reach
    /// `stop_at` (a merge point or loop exit that belongs to an outer scope).
    fn structure_from(
        &self,
        bb: usize,
        visited: &mut HashSet<usize>,
        stop_at: Option<usize>,
    ) -> Vec<CStmt> {
        if Some(bb) == stop_at {
            return Vec::new();
        }
        if bb >= self.func.blocks.len() {
            return Vec::new();
        }
        let blk = &self.func.blocks[bb];
        if blk.is_cleanup {
            return Vec::new();
        }
        if visited.contains(&bb) {
            // Back-edge: we've already structured this block as part of a
            // loop.  Emit a continue (back to loop header) or just return
            // empty (the while handles re-entry).
            return Vec::new();
        }

        // -- Loop header? Emit a while. --
        if self.loop_headers.contains(&bb) && !visited.contains(&bb) {
            return self.structure_loop(bb, visited, stop_at);
        }

        visited.insert(bb);

        let mut stmts = self.translate_statements(blk);
        let tail = self.structure_terminator(bb, &blk.terminator, visited, stop_at);
        stmts.extend(tail);
        stmts
    }

    fn structure_loop(
        &self,
        header: usize,
        visited: &mut HashSet<usize>,
        outer_stop: Option<usize>,
    ) -> Vec<CStmt> {
        let exit = self.loop_exits.get(&header).copied();
        let body_blocks = self.loop_bodies.get(&header);

        // Mark all body blocks as "in-loop" so inner structuring stops at
        // the header (which becomes a `continue`).
        let mut loop_visited = visited.clone();
        // Don't mark the header itself yet -- we need to structure it.

        let body_stmts = self.structure_loop_body(header, &mut loop_visited, exit, body_blocks);

        // Transfer loop_visited marks to outer visited.
        *visited = loop_visited;

        let mut result = vec![CStmt::While {
            cond: CExpr::Literal("true".to_string()),
            body: body_stmts,
        }];

        // Continue structuring after the loop exit.
        if let Some(exit_bb) = exit {
            result.extend(self.structure_from(exit_bb, visited, outer_stop));
        }

        result
    }

    fn structure_loop_body(
        &self,
        header: usize,
        visited: &mut HashSet<usize>,
        exit: Option<usize>,
        body_blocks: Option<&HashSet<usize>>,
    ) -> Vec<CStmt> {
        let blk = &self.func.blocks[header];
        visited.insert(header);

        let mut stmts = self.translate_statements(blk);

        match &blk.terminator {
            MirTerminator::SwitchInt {
                discr,
                targets,
                otherwise,
            } if targets.len() == 1 => {
                // Typical loop condition: switchInt(cond) -> [0: exit, _: body]
                // or [1: body, _: exit].
                let (val, val_bb) = targets[0];
                let other_bb = *otherwise;

                let (_cond_bb, continue_bb) = if self.is_exit(val_bb, exit, body_blocks) {
                    (val_bb, other_bb)
                } else {
                    (other_bb, val_bb)
                };

                // Emit: if (exit_cond) { break; }
                let exit_cond = if self.is_exit(val_bb, exit, body_blocks) {
                    format_switch_cond(discr, val, false)
                } else {
                    format_switch_cond(discr, val, true)
                };

                stmts.push(CStmt::If {
                    cond: exit_cond,
                    then_body: vec![CStmt::Break],
                    else_body: Vec::new(),
                });

                // Structure the continue side of the loop body.
                stmts.extend(self.structure_from(continue_bb, visited, exit));

                stmts
            }
            _ => {
                // Non-conditional loop (e.g. loop { ... }) or complex header.
                let tail =
                    self.structure_terminator(header, &blk.terminator, visited, exit);
                stmts.extend(tail);
                stmts
            }
        }
    }

    fn is_exit(&self, bb: usize, exit: Option<usize>, body: Option<&HashSet<usize>>) -> bool {
        if Some(bb) == exit {
            return true;
        }
        if let Some(body_set) = body {
            return !body_set.contains(&bb);
        }
        false
    }

    fn structure_terminator(
        &self,
        bb: usize,
        term: &MirTerminator,
        visited: &mut HashSet<usize>,
        stop_at: Option<usize>,
    ) -> Vec<CStmt> {
        match term {
            MirTerminator::Goto(target) => self.structure_from(*target, visited, stop_at),

            MirTerminator::Return => {
                vec![CStmt::Return(Some(CExpr::Local(0)))]
            }

            MirTerminator::Unreachable => {
                vec![CStmt::Comment("unreachable".to_string())]
            }

            MirTerminator::Drop {
                place, target, ..
            } => {
                let mut stmts = vec![CStmt::Drop {
                    place: place.clone(),
                }];
                stmts.extend(self.structure_from(*target, visited, stop_at));
                stmts
            }

            MirTerminator::Call {
                func,
                args,
                destination,
                target,
                ..
            } => {
                let mut stmts = vec![CStmt::Assign {
                    lhs: destination.clone(),
                    rhs: CExpr::Call {
                        func: func.clone(),
                        args: args.clone(),
                    },
                }];
                if let Some(tgt) = target {
                    stmts.extend(self.structure_from(*tgt, visited, stop_at));
                }
                stmts
            }

            MirTerminator::Assert {
                cond,
                expected,
                target,
                ..
            } => {
                let assert_cond = if *expected {
                    CExpr::UnaryOp {
                        op: "!".to_string(),
                        operand: Box::new(cond.clone()),
                    }
                } else {
                    cond.clone()
                };
                let mut stmts = vec![CStmt::If {
                    cond: assert_cond,
                    then_body: vec![CStmt::Comment("assertion failed -- panic".to_string())],
                    else_body: Vec::new(),
                }];
                stmts.extend(self.structure_from(*target, visited, stop_at));
                stmts
            }

            MirTerminator::SwitchInt {
                discr,
                targets,
                otherwise,
            } => self.structure_switch(bb, discr, targets, *otherwise, visited, stop_at),
        }
    }

    fn structure_switch(
        &self,
        _bb: usize,
        discr: &CExpr,
        targets: &[(u128, usize)],
        otherwise: usize,
        visited: &mut HashSet<usize>,
        stop_at: Option<usize>,
    ) -> Vec<CStmt> {
        if targets.len() == 1 {
            // Binary branch -> if/else.
            let (val, val_bb) = targets[0];
            let other_bb = otherwise;
            let merge = self.find_merge(val_bb, other_bb);

            let then_stmts = self.structure_from(val_bb, visited, merge.or(stop_at));
            let else_stmts = self.structure_from(other_bb, visited, merge.or(stop_at));

            // In MIR, switchInt often tests for 0 (false). We normalise so
            // that the "then" branch corresponds to the non-zero / true case.
            let (cond, then_body, else_body) = if val == 0 {
                // val==0 goes to val_bb, otherwise goes to other_bb
                // So val_bb is the "false" branch.
                (discr.clone(), else_stmts, then_stmts)
            } else {
                let cond = format_switch_cond(discr, val, false);
                (cond, then_stmts, else_stmts)
            };

            let mut stmts = vec![CStmt::If {
                cond,
                then_body,
                else_body,
            }];

            if let Some(merge_bb) = merge {
                stmts.extend(self.structure_from(merge_bb, visited, stop_at));
            }

            stmts
        } else {
            // Multi-arm switch.
            let all_targets: Vec<usize> =
                targets.iter().map(|(_, bb)| *bb).chain(Some(otherwise)).collect();
            let merge = self.find_merge_multi(&all_targets);

            let cases: Vec<(String, Vec<CStmt>)> = targets
                .iter()
                .map(|(val, bb)| {
                    let label = format!("{val}");
                    let body = self.structure_from(*bb, visited, merge.or(stop_at));
                    (label, body)
                })
                .collect();

            let default = self.structure_from(otherwise, visited, merge.or(stop_at));

            let mut stmts = vec![CStmt::Switch {
                scrutinee: discr.clone(),
                cases,
                default,
            }];

            if let Some(merge_bb) = merge {
                stmts.extend(self.structure_from(merge_bb, visited, stop_at));
            }

            stmts
        }
    }

    /// Find the merge point (reconvergence) for two branch targets.
    ///
    /// Uses forward-only BFS (excluding back-edges to loop headers) to avoid
    /// falsely identifying merge points through loop cycles.
    fn find_merge(&self, a: usize, b: usize) -> Option<usize> {
        let n = self.func.blocks.len();
        let reachable_a = bfs_forward_reachable(a, &self.successors, &self.loop_headers, n);
        let reachable_b = bfs_forward_reachable(b, &self.successors, &self.loop_headers, n);

        // The merge is the lowest-numbered block reachable from both
        // through forward edges, excluding the branch targets themselves
        // and cleanup blocks.
        for bb in 0..n {
            if bb == a || bb == b {
                continue;
            }
            if self.func.blocks[bb].is_cleanup {
                continue;
            }
            if reachable_a.contains(&bb) && reachable_b.contains(&bb) {
                return Some(bb);
            }
        }

        None
    }

    /// Find the merge point for multiple branch targets.
    fn find_merge_multi(&self, targets: &[usize]) -> Option<usize> {
        if targets.is_empty() {
            return None;
        }
        let n = self.func.blocks.len();
        let target_set: HashSet<usize> = targets.iter().copied().collect();
        let reachable_sets: Vec<HashSet<usize>> = targets
            .iter()
            .map(|&t| bfs_forward_reachable(t, &self.successors, &self.loop_headers, n))
            .collect();

        for bb in 0..n {
            if target_set.contains(&bb) {
                continue;
            }
            if self.func.blocks[bb].is_cleanup {
                continue;
            }
            if reachable_sets.iter().all(|s| s.contains(&bb)) {
                return Some(bb);
            }
        }
        None
    }

    fn translate_statements(&self, blk: &MirBlock) -> Vec<CStmt> {
        let mut stmts = Vec::new();
        for s in &blk.statements {
            match s {
                MirStmt::Assign { lhs, rhs } => {
                    stmts.push(CStmt::Assign {
                        lhs: lhs.clone(),
                        rhs: rhs.clone(),
                    });
                }
                MirStmt::StorageLive(_) | MirStmt::StorageDead(_) => {
                    // These are scope markers; we skip them in C output.
                }
                MirStmt::Other(s) => {
                    stmts.push(CStmt::Comment(s.clone()));
                }
            }
        }
        stmts
    }
}

// ---------------------------------------------------------------------------
// Graph algorithms
// ---------------------------------------------------------------------------

/// Non-recursive DFS to find back-edges in the CFG.
///
/// A back-edge is an edge `(u, v)` where `v` dominates `u` in a DFS tree,
/// meaning `v` is an ancestor of `u`.  The target `v` is a loop header.
fn find_back_edges(
    n: usize,
    successors: &[Vec<usize>],
) -> (HashSet<usize>, Vec<(usize, usize)>) {
    let mut headers = HashSet::new();
    let mut back_edges = Vec::new();

    // State: 0 = white (unvisited), 1 = gray (on stack), 2 = black (done).
    let mut color = vec![0u8; n];
    let mut stack: Vec<(usize, usize)> = Vec::new(); // (node, successor_index)

    color[0] = 1; // gray
    stack.push((0, 0));

    while let Some((node, idx)) = stack.last_mut() {
        let node = *node;
        if *idx >= successors[node].len() {
            color[node] = 2; // black
            stack.pop();
            continue;
        }
        let succ = successors[node][*idx];
        *idx += 1;

        match color[succ] {
            0 => {
                // Tree edge.
                color[succ] = 1;
                stack.push((succ, 0));
            }
            1 => {
                // Back-edge: succ is an ancestor on the current DFS path.
                headers.insert(succ);
                back_edges.push((node, succ));
            }
            _ => {
                // Cross or forward edge, ignore.
            }
        }
    }

    (headers, back_edges)
}

/// Compute the set of blocks in each natural loop.
///
/// For a back-edge `(source, header)`, the loop body is the set of blocks
/// that can reach `source` without going through `header`, plus `header`.
fn compute_loop_bodies(
    back_edges: &[(usize, usize)],
    successors: &[Vec<usize>],
    n: usize,
) -> HashMap<usize, HashSet<usize>> {
    // Build reverse adjacency list.
    let mut predecessors = vec![Vec::new(); n];
    for (u, succs) in successors.iter().enumerate() {
        for &v in succs {
            predecessors[v].push(u);
        }
    }

    let mut bodies: HashMap<usize, HashSet<usize>> = HashMap::new();

    for &(source, header) in back_edges {
        let body = bodies.entry(header).or_default();
        body.insert(header);
        if source != header {
            body.insert(source);
            // Walk backwards from source to find all blocks that can reach
            // source without leaving through header.
            let mut work = vec![source];
            while let Some(b) = work.pop() {
                for &pred in &predecessors[b] {
                    if pred != header && body.insert(pred) {
                        work.push(pred);
                    }
                }
            }
        }
    }

    bodies
}

/// For each loop, find the exit block: the first successor of any loop-body
/// block that is NOT in the loop body.
fn compute_loop_exits(
    loop_bodies: &HashMap<usize, HashSet<usize>>,
    successors: &[Vec<usize>],
) -> HashMap<usize, usize> {
    let mut exits = HashMap::new();

    for (&header, body) in loop_bodies {
        // Prefer the exit reachable from the header's branch (if the header
        // has a conditional that exits the loop).
        for &bb in body {
            for &succ in &successors[bb] {
                if !body.contains(&succ) {
                    exits.entry(header).or_insert(succ);
                }
            }
        }
    }

    exits
}

/// BFS from `start`, following only forward edges (skipping edges to loop
/// headers).  This prevents cycles from creating false merge points.
fn bfs_forward_reachable(
    start: usize,
    successors: &[Vec<usize>],
    loop_headers: &HashSet<usize>,
    n: usize,
) -> HashSet<usize> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start);
    queue.push_back(start);
    while let Some(bb) = queue.pop_front() {
        if bb >= n {
            continue;
        }
        for &succ in &successors[bb] {
            // Skip back-edges to loop headers (these create cycles that
            // confuse merge-point detection).
            if loop_headers.contains(&succ) {
                continue;
            }
            if visited.insert(succ) {
                queue.push_back(succ);
            }
        }
    }
    visited
}

/// Return the successor block indices for a terminator.
fn term_successors(term: &MirTerminator) -> Vec<usize> {
    match term {
        MirTerminator::Goto(t) => vec![*t],
        MirTerminator::Return | MirTerminator::Unreachable => Vec::new(),
        MirTerminator::SwitchInt {
            targets, otherwise, ..
        } => {
            let mut succs: Vec<usize> = targets.iter().map(|(_, bb)| *bb).collect();
            succs.push(*otherwise);
            succs
        }
        MirTerminator::Drop { target, .. } => vec![*target],
        MirTerminator::Call { target, .. } => {
            target.iter().copied().collect()
        }
        MirTerminator::Assert { target, .. } => vec![*target],
    }
}

fn format_switch_cond(discr: &CExpr, val: u128, negate: bool) -> CExpr {
    if negate {
        if val == 0 {
            discr.clone()
        } else {
            CExpr::BinOp {
                op: "!=".to_string(),
                lhs: Box::new(discr.clone()),
                rhs: Box::new(CExpr::Literal(format!("{val}"))),
            }
        }
    } else if val == 0 {
        CExpr::UnaryOp {
            op: "!".to_string(),
            operand: Box::new(discr.clone()),
        }
    } else {
        CExpr::BinOp {
            op: "==".to_string(),
            lhs: Box::new(discr.clone()),
            rhs: Box::new(CExpr::Literal(format!("{val}"))),
        }
    }
}
