use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{mapper, Page, PageTable, PhysFrame, Mapper, Size4KiB, FrameAllocator, OffsetPageTable},
    VirtAddr, PhysAddr,
};

pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static>
{
    let level_4_page_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_page_table, physical_memory_offset)
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_page_table_frame, _) = Cr3::read();

    let phys = level_4_page_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub fn create_example_mapping(page: Page, mapper: &mut OffsetPageTable, frame_allocator: &mut impl FrameAllocator<Size4KiB>)
{
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to failed").flush();
}

pub struct BootInfoFrameAllocator
{
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator
{
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self
    {
        BootInfoFrameAllocator
        {
            memory_map,
            next: 0,
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame>
    {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);

        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());

        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator
{
    fn allocate_frame(&mut self) -> Option<PhysFrame>
    {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackBounds
{
    start: VirtAddr,
    end: VirtAddr,
}


impl StackBounds
{
    pub fn start(&self) -> VirtAddr
    {
        self.start
    }
    pub fn end(&self) -> VirtAddr
    {
        self.end
    }
}

pub fn alloc_stack(size_in_pages: u64, mapper: &mut impl Mapper<Size4KiB>, frame_allocator: &mut impl FrameAllocator<Size4KiB>) -> Result<StackBounds, mapper::MapToError<Size4KiB>>
{
    use core::sync::atomic::{AtomicU64, Ordering};
    use x86_64::structures::paging::PageTableFlags as Flags;

    static STACK_ALLOC_NEXT: AtomicU64 = AtomicU64::new(0x5555_5555_0000);

    let guard_page_start = STACK_ALLOC_NEXT.fetch_add((size_in_pages + 1) * Page::<Size4KiB>::SIZE, Ordering::SeqCst);

    let guard_page = Page::from_start_address(VirtAddr::new(guard_page_start))
        .expect("`STACK_ALLOC_NEXT` not page aligned");

    let stack_start = guard_page + 1;
    let stack_end = stack_start + size_in_pages;

    let flags = Flags::PRESENT | Flags::WRITABLE;

    for page in Page::range(stack_start, stack_end)
    {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(mapper::MapToError::FrameAllocationFailed)?;
        unsafe {mapper.map_to(page, frame, flags, frame_allocator)?.flush();}
    }

    Ok(StackBounds
        {
            start: stack_start.start_address(),
            end: stack_end.start_address(),
        }
    )
}
