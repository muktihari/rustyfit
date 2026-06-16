use alloc::vec::Vec;

/// Lru is an opinionated `local_mesg_num` redefiner whose algorithm mimics
/// an LRU cache. When storage is full, the least recently used item is
/// replaced with a new item, which is then marked as the most recently used item.
/// This way, interleaving between `message definitions` is optimized.
pub(super) struct Lru {
    items: Vec<Vec<u8>>,
    bucket: Vec<u8>,
}

impl Lru {
    pub(super) const fn new() -> Self {
        Self {
            items: Vec::new(),
            bucket: Vec::new(),
        }
    }

    pub(super) fn reset(&mut self, n: usize) {
        self.bucket.clear();
        self.bucket.reserve_exact(n);
        self.items.resize(n, Vec::new());
    }

    pub(super) fn put(&mut self, item: &[u8]) -> (u8, bool) {
        if let Some(bucket_index) = self.bucket_index(item) {
            return (self.mark_as_recently_used(bucket_index), false);
        }
        if self.bucket.len() != self.items.len() {
            return (self.store(item), true);
        }
        (self.replace_least_recently_used(item), true)
    }

    fn store(&mut self, item: &[u8]) -> u8 {
        let item_index = self.bucket.len();
        self.items[item_index].clear();
        reserve_with_capacity_tiers(&mut self.items[item_index], item.len());
        self.items[item_index].extend_from_slice(item);
        self.bucket.push(item_index as u8);
        item_index as u8
    }

    fn mark_as_recently_used(&mut self, bucket_index: usize) -> u8 {
        let item_index = self.bucket[bucket_index];
        self.bucket.remove(bucket_index);
        self.bucket.push(item_index);
        item_index
    }

    fn replace_least_recently_used(&mut self, item: &[u8]) -> u8 {
        let item_index = self.bucket[0] as usize;
        self.bucket.remove(0);
        self.bucket.push(item_index as u8);
        self.items[item_index].clear();
        reserve_with_capacity_tiers(&mut self.items[item_index], item.len());
        self.items[item_index].extend_from_slice(item);
        item_index as u8
    }

    fn bucket_index(&self, item: &[u8]) -> Option<usize> {
        for (bucket_index, &item_index) in self.bucket.iter().enumerate().rev() {
            if self.items[item_index as usize] == item {
                return Some(bucket_index);
            }
        }
        None
    }
}

/// Prevent vector for growing more than 1537 bytes while also reducing
/// frequency of reallocation by reserving capacity in fixed-size tiers.
///
/// NOTE: Clear vector before passing.
fn reserve_with_capacity_tiers(v: &mut Vec<u8>, n: usize) {
    if n <= v.capacity() {
        return;
    }

    const RESERVED: usize = 7; // header, mesg_num, etc.
    const N: usize = 255 * 2; // n fields + n developer_fields
    const FULL: usize = RESERVED + N * 3;
    const THREE_QUARTER: usize = RESERVED + (N * 3 / 4) * 3;
    const HALF: usize = RESERVED + (N / 2) * 3;
    const QUARTER: usize = RESERVED + (N / 4) * 3;

    // Start at QUARTER because in practice, encountering more than 127 entries,
    // combination of fields and developer fields, is already rare. (127 * 3 = 381 bytes).
    // Session message has approx. 158 known fields, not all are used at the same time.
    // We probably never reallocate.
    let target = if n <= QUARTER {
        QUARTER
    } else if n <= HALF {
        HALF
    } else if n <= THREE_QUARTER {
        THREE_QUARTER
    } else {
        FULL
    };

    // Note that allocator may still give more space than requested.
    v.reserve_exact(target);
}

#[cfg(test)]
mod tests {
    use crate::encoder::{Lru, lru::reserve_with_capacity_tiers};
    use alloc::{vec, vec::Vec};

    #[test]
    fn test_lru() {
        const SIZE: usize = 16;
        let mut lru = Lru::new();
        lru.reset(SIZE);

        assert_eq!(lru.bucket.len(), 0);
        assert_eq!(lru.bucket.capacity(), SIZE);
        assert_eq!(lru.items.len(), SIZE);
        assert_eq!(lru.items.capacity(), SIZE);

        // place (size * 10) different items, the lru will be shifted in roundroubin order.
        for i in 0..SIZE * 10 {
            let mut b = vec![0u8; i + 1];
            b[0] = i as u8;
            let (local_mesg_num, is_new) = lru.put(&b);
            assert_eq!(local_mesg_num, (i % SIZE) as u8);
            assert!(is_new);
        }

        // put same items should shift the lru bucket
        for i in 0..SIZE {
            let item = lru.items[i].clone();
            let (local_mesg_num, _) = lru.put(&item);
            assert_eq!(local_mesg_num, i as u8);
            assert_eq!(lru.bucket[SIZE - 1], i as u8);
        }

        // check index exist
        assert_eq!(
            lru.bucket_index(&lru.items[lru.bucket[1] as usize]),
            Some(1)
        );

        // check index not exist
        assert!(lru.bucket_index(&[255, 255]).is_none());

        lru.reset(SIZE);
        assert_eq!(lru.bucket.len(), 0);
        assert_eq!(lru.items.len(), SIZE);
    }

    #[test]
    fn test_reserve_with_capacity_tiers() {
        let mut v = Vec::<u8>::new();

        reserve_with_capacity_tiers(&mut v, 10);
        assert_eq!(v.capacity(), 388);

        reserve_with_capacity_tiers(&mut v, 256);
        assert_eq!(v.capacity(), 388);

        reserve_with_capacity_tiers(&mut v, 510);
        assert_eq!(v.capacity(), 772);

        reserve_with_capacity_tiers(&mut v, 1000);
        assert_eq!(v.capacity(), 1153);

        reserve_with_capacity_tiers(&mut v, 1200);
        assert_eq!(v.capacity(), 1537);
    }
}
