use std::ffi::c_void;

// LIST IMPLEMENTATION DELETED (Now fully native in Pace)

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
    if map.contains_key(&key) { 1 } else { 0 }
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
pub extern "C" fn mapKeysLen(ptr: *mut c_void) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    map.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn mapKeyAt(ptr: *mut c_void, index: i64) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    if index < 0 || index as usize >= map.len() {
        return std::ptr::null_mut();
    }
    let key = *map.keys().nth(index as usize).unwrap();
    key as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn mapValueAt(ptr: *mut c_void, index: i64) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let map = unsafe { &mut *(ptr as *mut std::collections::HashMap<u64, *mut c_void>) };
    if index < 0 || index as usize >= map.len() {
        return std::ptr::null_mut();
    }
    *map.values().nth(index as usize).unwrap()
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
    if set.contains(&val) { 1 } else { 0 }
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
pub extern "C" fn setValuesLen(ptr: *mut c_void) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    set.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn setValueAt(ptr: *mut c_void, index: i64) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let set = unsafe { &mut *(ptr as *mut std::collections::HashSet<u64>) };
    if index < 0 || index as usize >= set.len() {
        return std::ptr::null_mut();
    }
    let val = *set.iter().nth(index as usize).unwrap();
    val as *mut c_void
}

// listFree DELETED
#[unsafe(no_mangle)]
pub extern "C" fn mapFree(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(
                ptr as *mut std::collections::HashMap<u64, *mut c_void>,
            ));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn setFree(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr as *mut std::collections::HashSet<u64>));
        }
    }
}
