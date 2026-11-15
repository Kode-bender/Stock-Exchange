use std::ops::{Index, IndexMut};

pub struct MemoryPool<T, const N: usize> {
    slots: Vec<T>,
    free_list: Vec<u32>,
}

impl<T, const N: usize> MemoryPool<T, N> {
    pub fn new() -> Self {
        const {
            assert!(N > 0, "MemoryPool capacity must be positive.");
            assert!(
                N < u32::MAX as usize,
                "MemoryPool capacity must fit a u32 handle."
            );
        }
        Self {
            slots: Vec::with_capacity(N),
            free_list: Vec::with_capacity(N),
        }
    }

    pub fn allocate(&mut self, value: T) -> Option<u32> {
        if let Some(handle) = self.free_list.pop() {
            self.slots[handle as usize] = value;
            return Some(handle);
        }
        if self.slots.len() < N {
            let handle = self.slots.len() as u32;
            self.slots.push(value);
            return Some(handle);
        }
        None
    }

    pub fn deallocate(&mut self, handle: u32) {
        self.free_list.push(handle);
    }
}

impl<T, const N: usize> Default for MemoryPool<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Index<u32> for MemoryPool<T, N> {
    type Output = T;

    fn index(&self, handle: u32) -> &T {
        &self.slots[handle as usize]
    }
}

impl<T, const N: usize> IndexMut<u32> for MemoryPool<T, N> {
    fn index_mut(&mut self, handle: u32) -> &mut T {
        &mut self.slots[handle as usize]
    }
}
