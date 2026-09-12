use std::hash::Hash;
#[derive(Debug)]
#[repr(C)]
pub struct HashNode<K, V> {
    // NonNull with Option is used since it's safer than raw pointers and lets the compiler optimize
    // for pointers not being null when used
    next: Option<NonNull<HashNode<K, V>>>,
    key: K,
    value: V,
    canary: u64,
}

// SAFETY: the map is only modified during single-threaded loading, and read during gameplay,
// modification and reading never happen simultanteously, as these 2 phases are separate.
unsafe impl<K, V> Send for UnorderedMap<K, V> {}
unsafe impl<K, V> Sync for UnorderedMap<K, V> {}

/// Essentially a HashMap but with stable memory on insert/remove, so we can give out raw pointers
/// that will remain valid
#[derive(Debug)]
pub struct UnorderedMap<K, V> {
    buckets: NonNull<Option<NonNull<HashNode<K, V>>>>,
    bucket_count: usize,
}
impl<K, V> UnorderedMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    pub fn new(bucket_count: usize) -> Self {
        // Calculate layout based on the actual type stored in the buckets
        let layout =
            Layout::array::<Option<NonNull<HashNode<K, V>>>>(bucket_count).expect("Layout error");

        unsafe {
            // Allocate and zero out the memory
            let raw_ptr = alloc_zeroed(layout) as *mut Option<NonNull<HashNode<K, V>>>;

            // Promote to NonNull
            let buckets = NonNull::new(raw_ptr).unwrap_or_else(|| {
                handle_alloc_error(layout);
            });

            UnorderedMap {
                buckets,
                bucket_count,
            }
        }
    }
    pub fn contains_key(&self, key: &K) -> bool {
        // Just check if get_unsafe returns Some
        self.get(key).is_some()
    }
    /// Returns true if not a duplicate.
    pub fn insert(&mut self, key: K, value: V) -> bool {
        let bkt = calculate_hash(&key, self.bucket_count);

        unsafe {
            // Use as_ptr() to get the array pointer, then index
            let bucket_ptr = self.buckets.as_ptr().add(bkt);
            let mut curr = *bucket_ptr;

            while let Some(mut node_ptr) = curr {
                if node_ptr.as_ref().key == key {
                    node_ptr.as_mut().value = value;
                    return false;
                }
                curr = node_ptr.as_ref().next;
            }

            let mut new_node = allocate_node(key, value);
            // new_node is NonNull, so access as_ptr()
            new_node.as_mut().next = *bucket_ptr;
            *bucket_ptr = Some(new_node);
        }
        true
    }
    /// Inserts a key-value pair. If the key already exists, updates the value
    /// and returns the old value
    /// Returns: (bool (true if newly inserted), *const V (pointer to the value))
    pub fn insert_or_assign(&mut self, key: K, value: V) -> (bool, *const V) {
        let bkt = calculate_hash(&key, self.bucket_count);

        unsafe {
            let bucket_ptr = self.buckets.as_ptr().add(bkt);
            let mut curr = *bucket_ptr;

            // 1. Search for existing key
            while let Some(mut node_ptr) = curr {
                if node_ptr.as_ref().key == key {
                    // Update existing value
                    node_ptr.as_mut().value = value;
                    // Return false (not newly inserted) and pointer to the value
                    return (false, &node_ptr.as_ref().value as *const V);
                }
                curr = node_ptr.as_ref().next;
            }

            // 2. Key not found, create new node
            let mut new_node = allocate_node(key, value);
            new_node.as_mut().next = *bucket_ptr;
            *bucket_ptr = Some(new_node);

            // Return true (inserted) and pointer to the value
            (true, &new_node.as_ref().value as *const V)
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let bkt = calculate_hash(key, self.bucket_count);

        unsafe {
            let mut curr = *self.buckets.as_ptr().add(bkt);
            while let Some(node_ptr) = curr {
                let node = node_ptr.as_ref();
                if node.key == *key {
                    return Some(&node.value);
                }
                curr = node.next;
            }
        }
        None
    }
    pub fn get_mut(&mut self, key: &K) -> Option<*mut V> {
        let bkt = calculate_hash(key, self.bucket_count);

        unsafe {
            let mut curr = *self.buckets.as_ptr().add(bkt);
            while let Some(mut node_ptr) = curr {
                let node = node_ptr.as_mut();
                if node.key == *key {
                    return Some(&mut node.value as *mut V);
                }
                curr = node.next;
            }
        }
        None
    }

    pub fn remove(&mut self, key: &K) -> bool {
        let bkt = calculate_hash(key, self.bucket_count);

        unsafe {
            let bucket_ptr = self.buckets.as_ptr().add(bkt);
            let mut curr = *bucket_ptr;
            let mut prev: Option<NonNull<HashNode<K, V>>> = None;

            while let Some(node_ptr) = curr {
                if node_ptr.as_ref().key == *key {
                    if let Some(mut prev_ptr) = prev {
                        prev_ptr.as_mut().next = node_ptr.as_ref().next;
                    } else {
                        *bucket_ptr = node_ptr.as_ref().next;
                    }
                    deallocate_node(node_ptr.as_ptr());
                    return true;
                }
                prev = Some(node_ptr);
                curr = node_ptr.as_ref().next;
            }
        }
        false
    }
}
fn calculate_hash<K: Hash>(key: &K, bucket_count: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish() as usize;

    // modulo to stay in array bounds
    hash % bucket_count
}
use std::alloc::{Layout, alloc, alloc_zeroed, handle_alloc_error};
use std::hash::{DefaultHasher, Hasher};
use std::ptr::{NonNull, write};

unsafe fn allocate_node<K, V>(key: K, value: V) -> NonNull<HashNode<K, V>> {
    let layout = Layout::new::<HashNode<K, V>>();

    unsafe {
        let raw_ptr = alloc(layout) as *mut HashNode<K, V>;

        if raw_ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        // initialize the node with a null 'next' pointer
        write(
            raw_ptr,
            HashNode {
                next: None,
                key,
                value,
                canary: 0xDEADBEEFCAFEBABE,
            },
        );
        NonNull::new_unchecked(raw_ptr)
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
            // Iterate through every bucket
            for i in 0..self.bucket_count {
                // Get the head of the chain
                let mut curr = *self.buckets.as_ptr().add(i);

                // traverse the list
                while let Some(node_ptr) = curr {
                    // Cache the next pointer before deallocating the current node
                    let next = node_ptr.as_ref().next;

                    // Deallocate the current node (K, V, and the allocation itself)
                    deallocate_node(node_ptr.as_ptr());

                    // Move to the next node
                    curr = next;
                }
            }

            // Deallocate the bucket array itself
            // Must use the same Layout type used in 'new'
            let layout =
                std::alloc::Layout::array::<Option<NonNull<HashNode<K, V>>>>(self.bucket_count)
                    .expect("Layout error");

            std::alloc::dealloc(self.buckets.as_ptr() as *mut u8, layout);
        }
    }
}
impl<K, V> Default for UnorderedMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    fn default() -> Self {
        Self::new(8)
    }
}
pub struct IterMut<'a, K, V> {
    curr: Option<NonNull<HashNode<K, V>>>,
    bucket_idx: usize,
    map: &'a mut UnorderedMap<K, V>,
}
pub struct Iter<'a, K, V> {
    curr: Option<NonNull<HashNode<K, V>>>,
    bucket_idx: usize,
    map: &'a UnorderedMap<K, V>,
}

impl<K, V> UnorderedMap<K, V> {
    pub fn values_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            curr: None,
            bucket_idx: 0,
            map: self,
        }
    }
    pub fn values(&self) -> Iter<'_, K, V> {
        Iter {
            curr: None,
            bucket_idx: 0,
            map: self,
        }
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        // 1. Advance if we have a current node
        if let Some(node_ptr) = self.curr {
            self.curr = unsafe { node_ptr.as_ref().next };
        }

        // 2. Search for the next available bucket
        while self.curr.is_none() && self.bucket_idx < self.map.bucket_count {
            unsafe {
                let head = *self.map.buckets.as_ptr().add(self.bucket_idx);
                self.bucket_idx += 1;
                if head.is_some() {
                    self.curr = head;
                }
            }
        }

        // 3. Return the value
        self.curr
            .map(|mut node_ptr| unsafe { &mut node_ptr.as_mut().value })
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        // 1. Advance if we have a current node
        if let Some(node_ptr) = self.curr {
            self.curr = unsafe { node_ptr.as_ref().next };
        }

        // 2. Search for the next available bucket
        while self.curr.is_none() && self.bucket_idx < self.map.bucket_count {
            unsafe {
                let head = *self.map.buckets.as_ptr().add(self.bucket_idx);
                self.bucket_idx += 1;
                if head.is_some() {
                    self.curr = head;
                }
            }
        }

        // 3. Return the value
        self.curr
            .map(|node_ptr| unsafe { &node_ptr.as_ref().value })
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
            map.insert(i, i * 10);
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
        map.insert("key", 1);
        map.insert("key", 2); // Should update, not add new node

        let mut count = 0;
        for _ in map.values_mut() {
            count += 1;
        }
        assert_eq!(count, 1, "Duplicate key created a new node!");
    }
    #[test]
    fn test_minimal_reproduction() {
        let mut map = UnorderedMap::new(10);

        // We will track the pointers we get
        let mut pointers = Vec::new();

        for i in 0..10 {
            map.insert(i, i);
            unsafe {
                let head = *map.buckets.as_ptr().add(i); // This returns Option<NonNull<...>>

                // To get the pointer for printing/tracking:
                if let Some(node_ptr) = head {
                    println!("After inserting {}, head is {:p}", i, node_ptr.as_ptr());

                    if pointers.contains(&node_ptr.as_ptr()) {
                        panic!("FATAL: Pointer {:p} reused!", node_ptr.as_ptr());
                    }
                    pointers.push(node_ptr.as_ptr());
                }
            }
        }
    }
}
