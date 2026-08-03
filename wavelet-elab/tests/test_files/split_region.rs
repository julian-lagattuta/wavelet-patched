// Regions written as a union of ranges, and arrays annotated more than once.

#[cap(A: uniq @ {0..5, 7..9})]
fn split_write<const N: usize>(A: &mut [i32; N]) {
    let v = 1;
    let i = 0;
    let j = 7;
    A[i] = v;
    A[j] = v;
    ()
}

// Index 6 falls in the hole between the two ranges.
#[cap(A: uniq @ {0..5, 7..9})]
fn split_write_hole<const N: usize>(A: &mut [i32; N]) {
    let v = 1;
    let i = 6;
    A[i] = v;
    ()
}

// Two annotations for the same array and permission are unioned.
#[cap(A: uniq @ 0..5, A: uniq @ 7..9)]
fn repeated_annotation<const N: usize>(A: &mut [i32; N]) {
    let v = 1;
    let i = 0;
    let j = 7;
    A[i] = v;
    A[j] = v;
    ()
}

// Two annotations with different permissions give the array both capabilities.
#[cap(A: uniq @ {0..5}, A: shrd @ {7..9})]
fn mixed_permissions<const N: usize>(A: &mut [i32; N]) {
    let v = 1;
    let i = 0;
    let j = 7;
    A[i] = v;
    let w = A[j];
    ()
}

// A bare index denotes a singleton region, and `{}` the empty region.
#[cap(A: uniq @ {3}, B: shrd @ {})]
fn singleton_region<const N: usize>(A: &mut [i32; N], B: &[i32; N]) {
    let v = 1;
    let i = 3;
    A[i] = v;
    ()
}
