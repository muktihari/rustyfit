pub(super) struct Lru {
    items: Vec<Vec<u8>>,
    bucket: Vec<usize>,
}

impl Lru {
    pub(super) fn new(n: usize) -> Self {
        Self {
            items: vec![Vec::new(); n],
            bucket: Vec::with_capacity(n),
        }
    }

    pub(super) fn reset(&mut self) {
        self.bucket.clear();
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
        self.items[item_index].extend_from_slice(item);
        self.bucket.push(item_index);
        item_index as u8
    }

    fn mark_as_recently_used(&mut self, bucket_index: usize) -> u8 {
        let item_index = self.bucket[bucket_index];
        self.bucket.remove(bucket_index);
        self.bucket.push(item_index);
        item_index as u8
    }

    fn replace_least_recently_used(&mut self, item: &[u8]) -> u8 {
        let item_index = self.bucket[0];
        self.bucket.remove(0);
        self.bucket.push(item_index);
        self.items[item_index].clear();
        self.items[item_index].extend_from_slice(item);
        item_index as u8
    }

    fn bucket_index(&self, item: &[u8]) -> Option<usize> {
        for i in (0..self.bucket.len()).rev() {
            let cur = self.bucket[i];
            if self.items[cur] == item {
                return Some(i);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::encoder::Lru;

    #[test]
    fn test_lru() {
        const SIZE: usize = 16;
        let mut lru = Lru::new(SIZE);

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
            assert_eq!(lru.bucket[SIZE - 1], i);
        }

        // check index exist
        assert_eq!(lru.bucket_index(&lru.items[lru.bucket[1]]), Some(1));

        // check index not exist
        assert!(lru.bucket_index(&[255, 255]).is_none());

        lru.reset();
        assert_eq!(lru.bucket.len(), 0);
        assert_eq!(lru.items.len(), SIZE);
    }
}
