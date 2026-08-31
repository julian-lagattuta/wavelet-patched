#[cap(callee_array: shrd @ i..4)]
fn read_suffix(i: usize, callee_array: &[i32; 4]) {
    let value = callee_array[i];
    ()
}

#[cap(caller_array: shrd @ 0..4)]
fn renamed_array_call(caller_array: &[i32; 4]) {
    let zero = 0;
    read_suffix(zero, caller_array)
}
