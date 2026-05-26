#[derive(Debug)]
#[repr(C)]
pub struct HashNode<K, V> {
    next: *mut HashNode<K, V>,
    key: K,
    value: V,
    canary: u64,
}

// SAFETY: the map is only modified during single-threaded loading, and read during gameplay,
// modification and reading never happen simultanteously, as these 2 phases are separate.
unsafe impl<K, V> Send for UnorderedMap<K, V> {}
unsafe impl<K, V> Sync for UnorderedMap<K, V> {}
#[derive(Debug)]
pub struct UnorderedMap<K, V> {
    buckets: *mut *mut HashNode<K, V>,
    bucket_count: usize,
}
impl<K, V> UnorderedMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    pub fn new(bucket_count: usize) -> Self {
        unsafe {
            let layout = std::alloc::Layout::array::<*mut HashNode<K, V>>(bucket_count)
                .expect("Layout error");

            let buckets = std::alloc::alloc_zeroed(layout) as *mut *mut HashNode<K, V>;

            if buckets.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            UnorderedMap {
                buckets,
                bucket_count,
            }
        }
    }
    /// Returns true if not a duplicate. Unsafe because this should never be used while in active gameplay, only during setup/loading
    /// of saves
    pub fn insert_unsafe(&mut self, key: K, value: V) -> bool {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = std::hash::Hasher::finish(&hasher) as usize;
        let bkt = hash % self.bucket_count;

        unsafe {
            // 1. Check for existing key to avoid duplicates/cycles
            let mut curr = *self.buckets.add(bkt);

            while !curr.is_null() {
                if (*curr).key == key {
                    // Key exists: update value in place and return
                    (*curr).value = value;
                    return false;
                }
                curr = (*curr).next;
            }

            let new_node = allocate_node(key, value);
            let old_head = *self.buckets.add(bkt);

            (*new_node).next = old_head;
            *self.buckets.add(bkt) = new_node;
        }
        true
    }
    pub fn get_unsafe(&self, key: &K) -> Option<*const V> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = std::hash::Hasher::finish(&hasher) as usize;

        // 2. Perform the lookup
        let bkt = hash % self.bucket_count;
        let mut curr = unsafe { *self.buckets.add(bkt) };

        while !curr.is_null() {
            unsafe {
                if (*curr).key == *key {
                    return Some(&(*curr).value as *const V);
                }
                curr = (*curr).next;
            }
        }
        None
    }
    pub fn remove_unsafe(&mut self, key: &K) -> bool {
        return false;
        // 1. Calculate hash internally
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish() as usize;
        let bkt = hash % self.bucket_count;

        unsafe {
            let mut curr = *self.buckets.add(bkt);
            let mut prev: *mut HashNode<K, V> = std::ptr::null_mut();

            while !curr.is_null() {
                // 2. Compare the key stored in the node with the input key
                if (*curr).key == *key {
                    // Unlink the node
                    if prev.is_null() {
                        *self.buckets.add(bkt) = (*curr).next;
                    } else {
                        (*prev).next = (*curr).next;
                    }

                    // Deallocate memory (runs destructors for K and V)
                    deallocate_node(curr);
                    return true;
                }
                prev = curr;
                curr = (*curr).next;
            }
        }
        false
    }
}
use std::alloc::{Layout, alloc};
use std::hash::Hasher;
use std::ptr::write;

unsafe fn allocate_node<K, V>(key: K, value: V) -> *mut HashNode<K, V> {
    let layout = Layout::new::<HashNode<K, V>>();

    unsafe {
        let raw_ptr = alloc(layout) as *mut HashNode<K, V>;

        if raw_ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        // We initialize the node with a null 'next' pointer
        write(
            raw_ptr,
            HashNode {
                next: std::ptr::null_mut(),
                key,
                value,
                canary: 0xDEADBEEFCAFEBABE,
            },
        );
        raw_ptr
    }
}
unsafe fn deallocate_node<K, V>(node_ptr: *mut HashNode<K, V>) {
    unsafe {
        std::ptr::drop_in_place(node_ptr);

        let layout = Layout::new::<HashNode<K, V>>();
        std::alloc::dealloc(node_ptr as *mut u8, layout);
    }
}
impl<K, V> Drop for UnorderedMap<K, V> {
    fn drop(&mut self) {
        unsafe {
            // 1. Iterate through every bucket and deallocate all nodes
            for i in 0..self.bucket_count {
                let mut curr = *self.buckets.add(i);
                while !curr.is_null() {
                    let next = (*curr).next;
                    deallocate_node(curr);
                    curr = next;
                }
            }

            // 2. Deallocate the bucket array itself
            let layout = std::alloc::Layout::array::<*mut HashNode<K, V>>(self.bucket_count)
                .expect("Layout error");
            std::alloc::dealloc(self.buckets as *mut u8, layout);
        }
    }
}
impl<K, V> Default for UnorderedMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    fn default() -> Self {
        // GCC typically uses a small prime number or a power of two
        // as the initial bucket count (e.g., 8 or 16).
        Self::new(8)
    }
}
pub struct IterMut<'a, K, V> {
    curr: *mut HashNode<K, V>,
    bucket_idx: usize,
    map: &'a mut UnorderedMap<K, V>,
}

impl<K, V> UnorderedMap<K, V> {
    pub fn values_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            curr: std::ptr::null_mut(),
            bucket_idx: 0,
            map: self,
        }
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        // 1. If we have a node, advance to the next one
        if !self.curr.is_null() {
            unsafe {
                self.curr = (*self.curr).next;
            }
        }

        // 2. If curr is now null, we have finished the chain
        //    or are starting. Search for the next available bucket.
        while self.curr.is_null() && self.bucket_idx < self.map.bucket_count {
            unsafe {
                // Peek at the bucket head
                let head = *self.map.buckets.add(self.bucket_idx);

                // We only take the head if we haven't finished this bucket yet.
                // If the iterator just hit null (finished a chain),
                // we move to the next bucket index.
                self.bucket_idx += 1;

                if !head.is_null() {
                    self.curr = head;
                    // We found a non-empty bucket; we stop searching now.
                    break;
                }
            }
        }

        // 3. Return the value
        if !self.curr.is_null() {
            unsafe { Some(&mut (*self.curr).value) }
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stress_insert_and_iterate() {
        let mut map = UnorderedMap::new(8);

        // Insert many elements to build a long linked list
        for i in 0..100 {
            map.insert_unsafe(i, i * 10);
        }

        // Verify counts
        let mut count = 0;
        for _ in map.values_mut() {
            count += 1;
        }

        assert_eq!(count, 100, "Iterator did not find all inserted elements!");
    }

    #[test]
    fn test_duplicate_update() {
        let mut map = UnorderedMap::new(8);
        map.insert_unsafe("key", 1);
        map.insert_unsafe("key", 2); // Should update, not add new node

        let mut count = 0;
        for _ in map.values_mut() {
            count += 1;
        }
        assert_eq!(count, 1, "Duplicate key created a new node!");
    }
    #[test]
    fn test_minimal_reproduction() {
        let mut map = UnorderedMap::new(1);

        // We will track the pointers we get
        let mut pointers = Vec::new();

        for i in 0..10 {
            map.insert_unsafe(i, i);
            // print the head of bucket 0
            unsafe {
                let head = *map.buckets.add(0);
                println!("After inserting {}, head is {:p}", i, head);

                // Check if we have seen this head before
                if pointers.contains(&head) {
                    panic!("FATAL: Pointer {:p} reused! Memory corruption!", head);
                }
                pointers.push(head);
            }
        }
    }
}
