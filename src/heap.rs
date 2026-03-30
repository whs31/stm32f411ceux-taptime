pub fn init() {
  use embedded_alloc::LlffHeap;

  #[global_allocator]
  static HEAP: LlffHeap = LlffHeap::empty();
  use core::mem::MaybeUninit;
  const HEAP_SIZE: usize = 1024;
  static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
  #[allow(static_mut_refs)]
  unsafe {
    HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE)
  }
}
