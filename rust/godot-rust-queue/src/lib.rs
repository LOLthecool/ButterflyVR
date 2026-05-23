use godot::prelude::*;
use std::collections::VecDeque;

struct GodotRustQueueExtension;

#[gdextension]
unsafe impl ExtensionLibrary for GodotRustQueueExtension {}

/// A Godot-friendly wrapper around Rust's [`std::collections::VecDeque`].
///
/// Provides O(1) push/pop at both ends and O(1) indexed access.
/// May provide a PackedArray deque implementation in the future.
#[derive(GodotClass)]
#[class(init, base = Node)]
struct Queue {
    inner: VecDeque<Variant>,
    base: Base<Node>,
}

#[godot_api]
impl Queue {
    #[func]
    fn push_back(&mut self, value: Variant) {
        self.inner.push_back(value);
    }

    #[func]
    fn push_front(&mut self, value: Variant) {
        self.inner.push_front(value);
    }

    #[func]
    fn pop_front(&mut self) -> Variant {
        self.inner.pop_front().unwrap_or_default()
    }

    #[func]
    fn pop_back(&mut self) -> Variant {
        self.inner.pop_back().unwrap_or_default()
    }

    #[func]
    fn front(&self) -> Variant {
        self.inner.front().cloned().unwrap_or_default()
    }

    #[func]
    fn back(&self) -> Variant {
        self.inner.back().cloned().unwrap_or_default()
    }

    #[func]
    fn get(&self, index: i64) -> Variant {
        if let Some(x) = self.inner.get(index as usize).cloned() {
            x
        } else {
            godot_error!(
                "invalid get index {index:?} on Queue of size {:?}",
                self.inner.len()
            );
            Variant::nil()
        }
    }

    #[func]
    fn set(&mut self, index: i64, value: Variant) {
        if let Some(slot) = self.inner.get_mut(index as usize) {
            *slot = value;
        } else {
            godot_error!(
                "invalid set index {index:?} on Queue of size {:?}",
                self.inner.len()
            );
        }
    }

    #[func]
    fn insert(&mut self, index: i64, value: Variant) {
        if let Ok(idx) = index.try_into() {
            self.inner.insert(idx, value);
        } else {
            godot_error!(
                "invalid insert index {index:?} on Queue of size {:?}",
                self.inner.len()
            );
        }
    }

    #[func]
    fn remove_at(&mut self, index: i64) -> Variant {
        if let Ok(idx) = index.try_into() {
            self.inner.remove(idx).unwrap()
        } else {
            godot_error!(
                "invalid insert index {index:?} on Queue of size {:?}",
                self.inner.len()
            );
            Variant::nil()
        }
    }

    #[func]
    fn size(&self) -> i64 {
        self.inner.len() as i64
    }

    #[func]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[func]
    fn clear(&mut self) {
        self.inner.clear();
    }

    #[func]
    fn contains(&self, value: Variant) -> bool {
        self.inner.iter().any(|v| *v == value)
    }

    #[func]
    fn find(&self, value: Variant) -> i64 {
        self.inner
            .iter()
            .position(|v| *v == value)
            .map(|i| i as i64)
            .unwrap_or(-1)
    }

    #[func]
    fn reserve(&mut self, additional: i64) {
        self.inner.reserve(additional as usize);
    }

    #[func]
    fn to_array(&self) -> Array<Variant> {
        let arr = Array::from_iter(self.inner.iter().cloned());
        arr
    }

    #[func]
    fn into_array(&mut self) -> Array<Variant> {
        let arr = Array::from_iter(self.inner.drain(0..self.inner.len()));
        arr
    }

    #[func]
    fn from_array(array: Array<Variant>) -> Gd<Self> {
        let mut queue = Self::new_alloc();
        queue.bind_mut().inner.clear();
        queue.bind_mut().inner.reserve(array.len());
        for i in 0..array.len() {
            queue.bind_mut().inner.push_back(array.at(i));
        }
        queue
    }

    #[func]
    fn copy_from_array(&mut self, array: Array<Variant>) {
        self.inner.clear();
        self.inner.reserve(array.len());
        for i in 0..array.len() {
            self.inner.push_back(array.at(i));
        }
    }
}
