use rtrb::{Consumer, Producer, RingBuffer};

pub struct SPSCQueue<T: Copy, const N: usize> {
    pub buffer: (Producer<T>, Consumer<T>),
}

impl<T: Copy, const N: usize> SPSCQueue<T, N> {
    pub fn new() -> Self {
        assert!(N > 1, "SPSCQueue requires at least two slots.");
        assert!(
            N.is_power_of_two(),
            "SPSCQueue capacity must be a power of two."
        );
        Self {
            buffer: RingBuffer::<T>::new(N),
        }
    }
}
