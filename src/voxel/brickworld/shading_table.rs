#[derive(Debug)]
pub struct ShadingBucket {
    global_offset: u32,
    slot_count: u32,
    slot_size: u32,
    free: Vec<u32>,
    used: Vec<u32>,
}

impl ShadingBucket {
    fn new(global_offset: u32, slot_count: u32, slot_size: u32) -> Self {
        let mut free = Vec::with_capacity(slot_count as usize);
        for i in (0..slot_count).rev() {
            free.push(i);
        }

        let used = Vec::with_capacity(slot_count as usize);
        Self {
            global_offset,
            slot_count,
            slot_size,
            free,
            used,
        }
    }

    fn contains_address(&self, address: u32) -> bool {
        let min = self.global_offset;
        let max = min + self.slot_count * self.slot_size;
        min <= address && address < max
    }

    fn try_alloc(&mut self) -> Option<u32> {
        // Mark the first free index as used
        let bucket_index = self.free.pop()?;
        self.used.push(bucket_index);

        // Convert the bucket index into a global address
        Some(self.global_offset + bucket_index * self.slot_size)
    }

    fn try_dealloc(&mut self, address: u32) -> Result<(), String> {
        log::trace!("Dealloc address: {}", address);
        if !self.contains_address(address) {
            let msg = format!("Address ({}) is not within bucket range.", address);
            return Err(msg);
        }

        let local_address = address - self.global_offset;
        if local_address % self.slot_size != 0 {
            return Err("Address is not aligned to bucket element size.".to_string());
        }

        let bucket_index = local_address / self.slot_size;
        if !self.used.contains(&bucket_index) {
            return Err("Address is not currently allocated.".to_string());
        }

        // All the potential errors are out of the way, time to actually deallocate
        let position = self.used.iter().position(|x| *x == bucket_index).unwrap();
        self.used.swap_remove(position);
        self.free.push(bucket_index);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ShadingTableAllocator {
    buckets: Vec<ShadingBucket>,
    bucket_count: u32,
    elements_per_bucket: u32,
    pub total_elements: u32,
    used_elements: u32,
}

impl ShadingTableAllocator {
    pub fn new(bucket_count: u32, elements_per_bucket: u32) -> Self {
        let total_elements = bucket_count * elements_per_bucket;
        let used_elements = 0;

        // Build the buckets. Ordered in ascending size
        let mut buckets = Vec::with_capacity(bucket_count as usize);
        for i in (0..bucket_count).rev() {
            let global_offset = i * elements_per_bucket;
            let slot_size = u32::pow(2, 9 - i);
            let slot_count = elements_per_bucket / slot_size;
            log::info!(
                "Creating bucket: offset({}), slot_size({}), slot_count({})",
                global_offset,
                slot_size,
                slot_count
            );
            buckets.push(ShadingBucket::new(global_offset, slot_count, slot_size));
        }

        Self {
            buckets,
            bucket_count,
            elements_per_bucket,
            total_elements,
            used_elements,
        }
    }

    pub fn try_alloc(&mut self, size: u32) -> Option<u32> {
        for i in 0..self.bucket_count as usize {
            let bucket = &mut self.buckets[i];
            if bucket.slot_size < size {
                continue;
            }

            let idx = bucket.try_alloc();
            if idx.is_some() {
                self.used_elements += bucket.slot_size;
                log::trace!(
                    "Allocated to shader table at {}. {}/{} ({}%)",
                    idx.unwrap(),
                    self.used_elements,
                    self.total_elements,
                    ((self.used_elements as f32 / self.total_elements as f32) * 100.0).floor()
                );
                return idx;
            }
        }

        None
    }

    pub fn try_dealloc(&mut self, address: u32) -> Result<(), String> {
        // Buckets are reverse order of their global offset so we need to reverse our idx
        let mut bucket_idx = address / self.elements_per_bucket;
        bucket_idx = self.bucket_count - bucket_idx - 1;
        let bucket = &mut self.buckets[bucket_idx as usize];
        self.used_elements -= bucket.slot_size;
        bucket.try_dealloc(address)
    }
}
