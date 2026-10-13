#[cap(A: uniq @ i..N)]
fn increment<const N: usize>(i: usize, A: &mut [i32; N]) {
    let c = i < N;
    if c {
        // p1: 0.5{i}, p2: 0.5{i}||1.0{i+1..N}
        let v = A[i];
        // p3, p_sync = join(p1, p2);
        let one_value = 1;
        let new_v = v + one_value;
        fence!();
        A[i] = new_v;
        let one_index = 1;
        let j = i + one_index;
        increment::<N>(j, A)
    } else {
        ()
    }
}
