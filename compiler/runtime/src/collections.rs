use std::ffi::c_void;

// LIST IMPLEMENTATION

#[unsafe(no_mangle)]
pub extern "C" fn listInit() -> *mut c_void {
    let vec: Vec<*mut c_void> = Vec::new();
    Box::into_raw(Box::new(vec)) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn listPush(ptr: *mut c_void, val: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut c_void>) };
    vec.push(val);
}

#[unsafe(no_mangle)]
pub extern "C" fn listPopRaw(ptr: *mut c_void) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut c_void>) };
    vec.pop().unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn listGetRaw(ptr: *mut c_void, index: i64) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut c_void>) };
    if index < 0 || index as usize >= vec.len() {
        return std::ptr::null_mut();
    }
    vec[index as usize]
}

#[unsafe(no_mangle)]
pub extern "C" fn listSet(ptr: *mut c_void, index: i64, val: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut c_void>) };
    if index >= 0 && (index as usize) < vec.len() {
        vec[index as usize] = val;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn listLen(ptr: *mut c_void) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut c_void>) };
    vec.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn listClear(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut c_void>) };
    vec.clear();
}

#[unsafe(no_mangle)]
pub extern "C" fn listRemoveRaw(ptr: *mut c_void, index: i64) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let vec = unsafe { &mut *(ptr as *mut Vec<*mut c_void>) };
    if index < 0 || index as usize >= vec.len() {
        return std::ptr::null_mut();
    }
    vec.remove(index as usize)
}


// MAP IMPLEMENTATION

#[unsafe(no_mangle)]
pub extern "C" fn mapInit() -> *mut c_void {
    let map: std::collections::HashMap<u64, *mut c_void> = std::collections::HashMap::new();
    Box::into_raw(Box::new(map)) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn mapSet(ptr: *mut c_void, key: u64, val: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    map.insert(key, val);
}

#[unsafe(no_mangle)]
pub extern "C" fn mapGetRaw(ptr: *mut c_void, key: u64) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    *map.get(&key).unwrap_or(&std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn mapRemove(ptr: *mut c_void, key: u64) {
    if ptr.is_null() {
        return;
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    map.remove(&key);
}

#[unsafe(no_mangle)]
pub extern "C" fn mapContains(ptr: *mut c_void, key: u64) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    if map.contains_key(&key) {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mapClear(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    map.clear();
}

#[unsafe(no_mangle)]
pub extern "C" fn mapKeysRaw(ptr: *mut c_void) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    let mut vec: Vec<*mut c_void> = Vec::new();
    for k in map.keys() {
        vec.push(*k as *mut c_void);
    }
    Box::into_raw(Box::new(vec)) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn mapValuesRaw(ptr: *mut c_void) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    let mut vec: Vec<*mut c_void> = Vec::new();
    for v in map.values() {
        vec.push(*v);
    }
    Box::into_raw(Box::new(vec)) as *mut c_void
}

// SET IMPLEMENTATION

#[unsafe(no_mangle)]
pub extern "C" fn setInit() -> *mut c_void {
    let set: std::collections::HashSet<u64> = std::collections::HashSet::new();
    Box::into_raw(Box::new(set)) as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn setInsert(ptr: *mut c_void, val: u64) {
    if ptr.is_null() {
        return;
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    set.insert(val);
}

#[unsafe(no_mangle)]
pub extern "C" fn setRemove(ptr: *mut c_void, val: u64) {
    if ptr.is_null() {
        return;
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    set.remove(&val);
}

#[unsafe(no_mangle)]
pub extern "C" fn setContains(ptr: *mut c_void, val: u64) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    if set.contains(&val) {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn setClear(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    set.clear();
}

#[unsafe(no_mangle)]
pub extern "C" fn setLen(ptr: *mut c_void) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    set.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn setValuesRaw(ptr: *mut c_void) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    let mut vec: Vec<*mut c_void> = Vec::new();
    for v in set.iter() {
        vec.push(*v as *mut c_void);
    }
    Box::into_raw(Box::new(vec)) as *mut c_void
}

