#[cap(A: uniq @ i..N)]
fn increment_bad<const N: usize>(i: usize, A: &mut [i32; N]) {
    let c = i < N;
    if c {
        let v = A[i];
        let one_value = 1;
        let new_v = v + one_value;
        A[i] = new_v;
        let one_index = 1;
        let j = i + one_index;
        increment_bad::<N>(j, A)
    } else {
        ()
    }
}
