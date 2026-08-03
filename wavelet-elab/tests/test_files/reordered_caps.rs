// The callee's annotations are written in the opposite order to its array
// parameters; call sites must still demand the right capability per array.

#[cap(dest: uniq @ 0..N, src: shrd @ 0..N)]
fn copy_all<const N: usize>(src: &[i32; N], dest: &mut [i32; N]) {
    let i = 0;
    let c = i < N;
    if c {
        let v = src[i];
        dest[i] = v;
        ()
    } else {
        ()
    }
}

#[cap(src: shrd @ 0..N, dest: uniq @ 0..N)]
fn copy_entry<const N: usize>(src: &[i32; N], dest: &mut [i32; N]) {
    copy_all::<N>(src, dest)
}
