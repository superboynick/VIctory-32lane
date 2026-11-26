//! Opt-in, thread-local nested timings. No code exists without resource-detail.
use std::{cell::RefCell, time::Instant};
const N: usize = 20;
const NAMES: [&str; N] = ["exec", "readable", "memory_ready", "placed", "buffer_lookup",
    "refresh", "gpu_writes", "texture", "heap", "roots", "memory_lock", "pages",
    "views", "copies", "deferred_lock", "texture_upload", "twin", "state", "debug_desc", "uav_note"];
#[derive(Clone, Copy)]
struct Batch { calls: [u64; N], ns: [u64; N], own: [u64; N] }
impl Batch { const EMPTY: Self = Self { calls: [0; N], ns: [0; N], own: [0; N] }; }
struct State { context: Option<usize>, depth: usize, spent: u64, batches: [Batch; 2] }
thread_local! { static STATE: RefCell<State> = const { RefCell::new(State { context: None, depth: 0, spent: 0, batches: [Batch::EMPTY; 2] }) }; }
pub(crate) struct Scope(Option<(Instant, u64, usize, usize)>);
impl Scope {
    pub(crate) fn new(kind: usize) -> Self {
        Self(STATE.with(|s| {
            let mut s = s.borrow_mut();
            let c = s.context?;
            if kind == 14 && s.depth == 0 { return None; }
            if matches!(kind, 1 | 2) { s.depth += 1; }
            Some((Instant::now(), s.spent, kind, c))
        }))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let Some((start, before, kind, context)) = self.0 else { return };
        let elapsed = start.elapsed().as_nanos() as u64;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if matches!(kind, 1 | 2) { s.depth -= 1; }
            let own = elapsed.saturating_sub(s.spent.wrapping_sub(before));
            s.spent = s.spent.wrapping_add(own);
            let b = &mut s.batches[context];
            b.calls[kind] += 1;
            b.ns[kind] += elapsed;
            b.own[kind] += own;
        });
    }
}
pub(crate) struct Root { scope: Option<Scope>, previous: Option<usize>, context: usize }
impl Root {
    pub(crate) fn new(context: usize) -> Self {
        let previous = STATE.with(|s| s.borrow_mut().context.replace(context));
        Self { scope: Some(Scope::new(0)), previous, context }
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        drop(self.scope.take());
        let batch = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.context = self.previous;
            let b = &mut s.batches[self.context];
            (b.calls[0] >= 4096).then(|| std::mem::replace(b, Batch::EMPTY))
        });
        if let Some(b) = batch {
            let site = if self.context == 0 { "dispatch" } else { "indirect" };
            for (i, part) in NAMES.iter().enumerate() {
                tracing::info!(site, part, calls = b.calls[i], ns = b.ns[i], own_ns = b.own[i], "resource detail");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_scopes_balance_and_do_not_leak_between_command_types() {
        STATE.with(|s| *s.borrow_mut() = State { context: None, depth: 0, spent: 0, batches: [Batch::EMPTY; 2] });
        for context in 0..2 {
            let _root = Root::new(context);
            drop(Scope::new(14)); // Outside resource preparation: not counted.
            {
                let _readable = Scope::new(1);
                let _lookup = Scope::new(4);
                let _lock = Scope::new(14);
            }
        }
        drop(Scope::new(1)); // Outside both commands: not counted.
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.context, None);
            assert_eq!(s.depth, 0);
            for b in s.batches {
                assert_eq!((b.calls[0], b.calls[1], b.calls[4], b.calls[14]), (1, 1, 1, 1));
                assert_eq!(b.own.iter().sum::<u64>(), b.ns[0]);
                assert!(b.ns[1] >= b.ns[4] && b.ns[4] >= b.ns[14]);
            }
        });
    }
}
