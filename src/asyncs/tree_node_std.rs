//! This mod provides the std-mutex implementation for the inner tree structure
//! of the `CancellationToken`.
//!
//! The locking strategy mirrors `tree_node.rs`: a node's parent is always locked
//! before the node itself, which is safe because children are always younger than
//! their parents.

use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

/// A node of the cancellation tree structure.
pub(crate) struct TreeNode {
    is_cancelled: AtomicBool,
    inner: Mutex<Inner>,
    waker: tokio::sync::Notify,
}

impl TreeNode {
    pub(crate) fn new() -> Self {
        Self::new_with_state(false, None, 0)
    }

    fn new_with_state(
        is_cancelled: bool,
        parent: Option<Arc<TreeNode>>,
        parent_idx: usize,
    ) -> Self {
        Self {
            is_cancelled: AtomicBool::new(is_cancelled),
            inner: Mutex::new(Inner {
                parent,
                parent_idx,
                children: vec![],
                num_handles: 1,
            }),
            waker: tokio::sync::Notify::new(),
        }
    }

    pub(crate) fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.waker.notified()
    }
}

/// The data contained inside a `TreeNode`.
struct Inner {
    parent: Option<Arc<TreeNode>>,
    parent_idx: usize,
    children: Vec<Arc<TreeNode>>,
    num_handles: usize,
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn try_lock_unpoisoned<T>(mutex: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    match mutex.try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(err)) => Some(err.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

fn new_cancelled_node() -> Arc<TreeNode> {
    Arc::new(TreeNode::new_with_state(true, None, 0))
}

/// Returns whether or not the node is cancelled.
pub(crate) fn is_cancelled(node: &Arc<TreeNode>) -> bool {
    node.is_cancelled.load(Relaxed)
}

/// Creates a child node.
pub(crate) fn child_node(parent: &Arc<TreeNode>) -> Arc<TreeNode> {
    // Fast path for already-cancelled parents.
    if parent.is_cancelled.load(Relaxed) {
        return new_cancelled_node();
    }

    let mut locked_parent = lock_unpoisoned(&parent.inner);
    // The parent may have been cancelled after the fast-path check while we were
    // waiting for its mutex.
    if parent.is_cancelled.load(Relaxed) {
        return new_cancelled_node();
    }

    let child = Arc::new(TreeNode::new_with_state(
        false,
        Some(parent.clone()),
        locked_parent.children.len(),
    ));

    locked_parent.children.push(child.clone());

    child
}

/// Disconnects the given parent from all of its children.
fn disconnect_children(node: &mut Inner) {
    for child in std::mem::take(&mut node.children) {
        let mut locked_child = lock_unpoisoned(&child.inner);
        locked_child.parent_idx = 0;
        locked_child.parent = None;
    }
}

/// Figures out the parent of the node and locks the node and its parent
/// atomically.
fn with_locked_node_and_parent<F, Ret>(node: &Arc<TreeNode>, func: F) -> Ret
where
    F: FnOnce(MutexGuard<'_, Inner>, Option<MutexGuard<'_, Inner>>) -> Ret,
{
    let mut locked_node = lock_unpoisoned(&node.inner);

    loop {
        let potential_parent = match locked_node.parent.as_ref() {
            Some(potential_parent) => potential_parent.clone(),
            None => return func(locked_node, None),
        };

        let locked_parent = match try_lock_unpoisoned(&potential_parent.inner) {
            Some(locked_parent) => locked_parent,
            None => {
                drop(locked_node);
                let locked_parent = lock_unpoisoned(&potential_parent.inner);
                locked_node = lock_unpoisoned(&node.inner);
                locked_parent
            }
        };

        if let Some(actual_parent) = locked_node.parent.as_ref() {
            if Arc::ptr_eq(actual_parent, &potential_parent) {
                return func(locked_node, Some(locked_parent));
            }
        }
    }
}

/// Moves all children from `node` to `parent`.
fn move_children_to_parent(node: &mut Inner, parent: &mut Inner) {
    parent.children.reserve(node.children.len());

    for child in std::mem::take(&mut node.children) {
        {
            let mut child_locked = lock_unpoisoned(&child.inner);
            child_locked.parent.clone_from(&node.parent);
            child_locked.parent_idx = parent.children.len();
        }
        parent.children.push(child);
    }
}

/// Removes a child from the parent.
fn remove_child(parent: &mut Inner, mut node: MutexGuard<'_, Inner>) {
    let pos = node.parent_idx;
    node.parent = None;
    node.parent_idx = 0;

    // Unlock node so that only one child at a time is locked.
    drop(node);

    if parent.children.len() == pos + 1 {
        parent.children.pop().unwrap();
    } else {
        let replacement_child = parent.children.pop().unwrap();
        lock_unpoisoned(&replacement_child.inner).parent_idx = pos;
        parent.children[pos] = replacement_child;
    }

    let len = parent.children.len();
    if 4 * len <= parent.children.capacity() {
        parent.children.shrink_to(2 * len);
    }
}

/// Increases the reference count of handles.
pub(crate) fn increase_handle_refcount(node: &Arc<TreeNode>) {
    let mut locked_node = lock_unpoisoned(&node.inner);

    // Once no handles are left over, the node gets detached from the tree.
    // There should never be a new handle once all handles are dropped.
    assert!(locked_node.num_handles > 0);

    locked_node.num_handles += 1;
}

/// Decreases the reference count of handles.
pub(crate) fn decrease_handle_refcount(node: &Arc<TreeNode>) {
    let num_handles = {
        let mut locked_node = lock_unpoisoned(&node.inner);
        assert!(locked_node.num_handles > 0);
        locked_node.num_handles -= 1;
        locked_node.num_handles
    };

    if num_handles == 0 {
        with_locked_node_and_parent(node, |mut node, parent| match parent {
            Some(mut parent) => {
                move_children_to_parent(&mut node, &mut parent);
                remove_child(&mut parent, node);
            }
            None => {
                disconnect_children(&mut node);
            }
        });
    }
}

/// Cancels a node and its children.
pub(crate) fn cancel(node: &Arc<TreeNode>) {
    if node.is_cancelled.load(Relaxed) {
        return;
    }

    let mut locked_node = lock_unpoisoned(&node.inner);
    if node.is_cancelled.load(Relaxed) {
        return;
    }

    // One by one, adopt grandchildren and then cancel and detach the child.
    while let Some(child) = locked_node.children.pop() {
        let mut locked_child = lock_unpoisoned(&child.inner);

        locked_child.parent = None;
        locked_child.parent_idx = 0;

        if child.is_cancelled.load(Relaxed) {
            continue;
        }

        while let Some(grandchild) = locked_child.children.pop() {
            let mut locked_grandchild = lock_unpoisoned(&grandchild.inner);

            locked_grandchild.parent = None;
            locked_grandchild.parent_idx = 0;

            if grandchild.is_cancelled.load(Relaxed) {
                continue;
            }

            if locked_grandchild.children.is_empty() {
                grandchild.is_cancelled.store(true, Relaxed);
                locked_grandchild.children = Vec::new();
                drop(locked_grandchild);
                grandchild.waker.notify_waiters();
            } else {
                locked_grandchild.parent = Some(node.clone());
                locked_grandchild.parent_idx = locked_node.children.len();
                drop(locked_grandchild);
                locked_node.children.push(grandchild);
            }
        }

        child.is_cancelled.store(true, Relaxed);
        locked_child.children = Vec::new();
        drop(locked_child);
        child.waker.notify_waiters();
    }

    node.is_cancelled.store(true, Relaxed);
    locked_node.children = Vec::new();
    drop(locked_node);
    node.waker.notify_waiters();
}

#[repr(transparent)]
pub(crate) struct MaybeDangling<T>(MaybeUninit<T>);

impl<T> Drop for MaybeDangling<T> {
    fn drop(&mut self) {
        // Safety: `0` is always initialized.
        unsafe { core::ptr::drop_in_place(self.0.as_mut_ptr()) };
    }
}

impl<T> MaybeDangling<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self(MaybeUninit::new(inner))
    }
}

impl<F: Future> Future for MaybeDangling<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Safety: `0` is always initialized.
        let fut = unsafe { self.map_unchecked_mut(|this| this.0.assume_init_mut()) };
        fut.poll(cx)
    }
}
