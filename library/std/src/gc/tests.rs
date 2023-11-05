use super::*;

#[test]
fn test_dispatchable() {
    struct S1 {
        x: u64,
    }
    struct S2 {
        y: u64,
    }
    trait T: Send {
        fn f(&self) -> u64;
    }
    impl T for S1 {
        fn f(&self) -> u64 {
            self.x
        }
    }
    impl T for S2 {
        fn f(&self) -> u64 {
            self.y
        }
    }

    let s1 = S1 { x: 1 };
    let s2 = S2 { y: 2 };
    let s1gc: Gc<S1> = Gc::new(s1);
    let s2gc: Gc<S2> = Gc::new(s2);
    assert_eq!(s1gc.f(), 1);
    assert_eq!(s2gc.f(), 2);

    let s1gcd: Gc<dyn T> = s1gc;
    let s2gcd: Gc<dyn T> = s2gc;
    assert_eq!(s1gcd.f(), 1);
    assert_eq!(s2gcd.f(), 2);
}
