struct HasDrop;

impl Drop for HasDrop {
    fn drop(&mut self) {}
}

struct HasDropNoFinalize;

impl Drop for HasDropNoFinalize {
    fn drop(&mut self) {}
}

struct FinalizedContainer<T>(T);
struct MaybeFinalize<T>(T);
struct ExplicitNoFinalize;

// This struct doesn't need finalizing, but it's not annoted as such.
struct NonAnnotated(usize);

unsafe impl std::gc::DropMethodFinalizerElidable for HasDropNoFinalize {}

impl<T> Drop for FinalizedContainer<T> {
    fn drop(&mut self) {}
}
