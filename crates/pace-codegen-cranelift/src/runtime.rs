use cranelift::prelude::*;
use cranelift_module::{FuncId, Linkage, Module};
use std::collections::HashMap;

pub fn declare_runtime_functions<M: Module>(
    module: &mut M,
    ptr_ty: Type,
) -> HashMap<ustr::Ustr, FuncId> {
    let mut sig_int = module.make_signature();
    sig_int.params.push(AbiParam::new(types::I64));
    let print_int_id = module
        .declare_function("__pace_print_int", Linkage::Import, &sig_int)
        .unwrap();

    let mut sig_float = module.make_signature();
    sig_float.params.push(AbiParam::new(types::F64));
    let print_float_id = module
        .declare_function("__pace_print_float", Linkage::Import, &sig_float)
        .unwrap();

    let mut sig_string = module.make_signature();
    sig_string.params.push(AbiParam::new(ptr_ty));
    let print_string_id = module
        .declare_function("__pace_print_string", Linkage::Import, &sig_string)
        .unwrap();

    let mut sig_malloc = module.make_signature();
    sig_malloc.params.push(AbiParam::new(types::I64));
    sig_malloc.returns.push(AbiParam::new(ptr_ty));
    let malloc_id = module
        .declare_function("__pace_malloc", Linkage::Import, &sig_malloc)
        .unwrap();

    let mut sig_noop = module.make_signature();
    sig_noop.params.push(AbiParam::new(ptr_ty));
    let noop_id = module
        .declare_function("__pace_noop", Linkage::Import, &sig_noop)
        .unwrap();

    let mut sig_retain = module.make_signature();
    sig_retain.params.push(AbiParam::new(ptr_ty));
    let retain_id = module
        .declare_function("__pace_retain", Linkage::Import, &sig_retain)
        .unwrap();

    let mut sig_release = module.make_signature();
    sig_release.params.push(AbiParam::new(ptr_ty));
    let release_id = module
        .declare_function("__pace_release", Linkage::Import, &sig_release)
        .unwrap();

    let mut sig_concat = module.make_signature();
    sig_concat.params.push(AbiParam::new(ptr_ty));
    sig_concat.params.push(AbiParam::new(ptr_ty));
    sig_concat.returns.push(AbiParam::new(ptr_ty));
    let concat_id = module
        .declare_function("__pace_concat_strings", Linkage::Import, &sig_concat)
        .unwrap();

    let mut sig_int_to_string = module.make_signature();
    sig_int_to_string.params.push(AbiParam::new(types::I64));
    sig_int_to_string.returns.push(AbiParam::new(ptr_ty));
    let int_to_str_id = module
        .declare_function("__pace_int_to_string", Linkage::Import, &sig_int_to_string)
        .unwrap();

    let mut sig_float_to_string = module.make_signature();
    sig_float_to_string.params.push(AbiParam::new(types::F64));
    sig_float_to_string.returns.push(AbiParam::new(ptr_ty));
    let float_to_str_id = module
        .declare_function(
            "__pace_float_to_string",
            Linkage::Import,
            &sig_float_to_string,
        )
        .unwrap();

    let mut sig_bool_to_string = module.make_signature();
    sig_bool_to_string.params.push(AbiParam::new(types::I64));
    sig_bool_to_string.returns.push(AbiParam::new(ptr_ty));
    let bool_to_str_id = module
        .declare_function(
            "__pace_bool_to_string",
            Linkage::Import,
            &sig_bool_to_string,
        )
        .unwrap();

    let mut sig_free = module.make_signature();
    sig_free.params.push(AbiParam::new(ptr_ty));
    sig_free.params.push(AbiParam::new(types::I64));
    let free_id = module
        .declare_function("__pace_free", Linkage::Import, &sig_free)
        .unwrap();

    let mut sig_ptr_store = module.make_signature();
    sig_ptr_store.params.push(AbiParam::new(ptr_ty));
    sig_ptr_store.params.push(AbiParam::new(types::I64));
    sig_ptr_store.params.push(AbiParam::new(types::I64));
    let ptr_store_id = module
        .declare_function("__pace_ptr_store", Linkage::Import, &sig_ptr_store)
        .unwrap();

    let mut sig_ptr_store8 = module.make_signature();
    sig_ptr_store8.params.push(AbiParam::new(ptr_ty));
    sig_ptr_store8.params.push(AbiParam::new(types::I64));
    sig_ptr_store8.params.push(AbiParam::new(types::I64));
    let ptr_store8_id = module
        .declare_function("__pace_ptr_store8", Linkage::Import, &sig_ptr_store8)
        .unwrap();

    let mut sig_ptr_load = module.make_signature();
    sig_ptr_load.params.push(AbiParam::new(ptr_ty));
    sig_ptr_load.params.push(AbiParam::new(types::I64));
    sig_ptr_load.returns.push(AbiParam::new(types::I64));
    let ptr_load_id = module
        .declare_function("__pace_ptr_load", Linkage::Import, &sig_ptr_load)
        .unwrap();

    let mut sig_ptr_load8 = module.make_signature();
    sig_ptr_load8.params.push(AbiParam::new(ptr_ty));
    sig_ptr_load8.params.push(AbiParam::new(types::I64));
    sig_ptr_load8.returns.push(AbiParam::new(types::I64));
    let ptr_load8_id = module
        .declare_function("__pace_ptr_load8", Linkage::Import, &sig_ptr_load8)
        .unwrap();

    let mut sig_time = module.make_signature();
    sig_time.params.push(AbiParam::new(types::I64));
    sig_time.returns.push(AbiParam::new(types::I64));
    let time_id = module
        .declare_function("__pace_time", Linkage::Import, &sig_time)
        .unwrap();

    let mut sig_get_year = module.make_signature();
    sig_get_year.params.push(AbiParam::new(types::I64));
    sig_get_year.returns.push(AbiParam::new(types::I64));
    let get_year_id = module
        .declare_function("__pace_get_year", Linkage::Import, &sig_get_year)
        .unwrap();

    let mut sig_hash = module.make_signature();
    sig_hash.params.push(AbiParam::new(types::I64));
    sig_hash.returns.push(AbiParam::new(types::I64));
    let hash_id = module
        .declare_function("__pace_hash", Linkage::Import, &sig_hash)
        .unwrap();


    // Actor runtime
    let mut sig_mb_create = module.make_signature();
    sig_mb_create.returns.push(AbiParam::new(ptr_ty));
    let mb_create_id = module
        .declare_function("__pace_mailbox_create", Linkage::Import, &sig_mb_create)
        .unwrap();

    let mut sig_mb_send = module.make_signature();
    sig_mb_send.params.push(AbiParam::new(ptr_ty)); // mb
    sig_mb_send.params.push(AbiParam::new(ptr_ty)); // func pointer
    sig_mb_send.params.push(AbiParam::new(types::I64)); // arg
    sig_mb_send.params.push(AbiParam::new(ptr_ty)); // promise
    let mb_send_id = module
        .declare_function("__pace_mailbox_send", Linkage::Import, &sig_mb_send)
        .unwrap();

    let mut sig_is_err = module.make_signature();
    sig_is_err.params.push(AbiParam::new(ptr_ty)); // ptr
    sig_is_err.returns.push(AbiParam::new(types::I64)); // bool (i64)
    let is_err_id = module
        .declare_function("__pace_is_err", Linkage::Import, &sig_is_err)
        .unwrap();
    let mut sig_mb_destroy = module.make_signature();
    sig_mb_destroy.params.push(AbiParam::new(ptr_ty));
    let mb_destroy_id = module
        .declare_function("__pace_mailbox_destroy", Linkage::Import, &sig_mb_destroy)
        .unwrap();

    let mut sig_prom_create = module.make_signature();
    sig_prom_create.returns.push(AbiParam::new(ptr_ty));
    let prom_create_id = module
        .declare_function("__pace_promise_create", Linkage::Import, &sig_prom_create)
        .unwrap();

    let mut sig_prom_resolve = module.make_signature();
    sig_prom_resolve.params.push(AbiParam::new(ptr_ty)); // promise
    sig_prom_resolve.params.push(AbiParam::new(types::I64)); // value
    let prom_resolve_id = module
        .declare_function("__pace_promise_resolve", Linkage::Import, &sig_prom_resolve)
        .unwrap();

    let mut sig_prom_await = module.make_signature();
    sig_prom_await.params.push(AbiParam::new(ptr_ty)); // promise
    sig_prom_await.returns.push(AbiParam::new(types::I64)); // value
    let prom_await_id = module
        .declare_function("__pace_promise_await", Linkage::Import, &sig_prom_await)
        .unwrap();

    let mut funcs = HashMap::new();
    funcs.insert(ustr::Ustr::from("print_int"), print_int_id);
    funcs.insert(ustr::Ustr::from("print_float"), print_float_id);
    funcs.insert(ustr::Ustr::from("print_string"), print_string_id);
    funcs.insert(ustr::Ustr::from("malloc"), malloc_id);
    funcs.insert(ustr::Ustr::from("retain"), retain_id);
    funcs.insert(ustr::Ustr::from("release"), release_id);
    funcs.insert(ustr::Ustr::from("__pace_noop"), noop_id);
    funcs.insert(ustr::Ustr::from("concat_strings"), concat_id);
    funcs.insert(ustr::Ustr::from("int_to_string"), int_to_str_id);
    funcs.insert(ustr::Ustr::from("float_to_string"), float_to_str_id);
    funcs.insert(ustr::Ustr::from("bool_to_string"), bool_to_str_id);
    funcs.insert(ustr::Ustr::from("free"), free_id);
    funcs.insert(ustr::Ustr::from("ptrStore"), ptr_store_id);
    funcs.insert(ustr::Ustr::from("ptrStore8"), ptr_store8_id);
    funcs.insert(ustr::Ustr::from("ptrLoad"), ptr_load_id);
    funcs.insert(ustr::Ustr::from("ptrLoad8"), ptr_load8_id);
    funcs.insert(ustr::Ustr::from("time"), time_id);
    funcs.insert(ustr::Ustr::from("getYear"), get_year_id);
    funcs.insert(ustr::Ustr::from("hash"), hash_id);

    funcs.insert(ustr::Ustr::from("__pace_mailbox_create"), mb_create_id);
    funcs.insert(ustr::Ustr::from("__pace_mailbox_send"), mb_send_id);
    funcs.insert(ustr::Ustr::from("__pace_mailbox_destroy"), mb_destroy_id);
    funcs.insert(ustr::Ustr::from("__pace_promise_create"), prom_create_id);
    funcs.insert(ustr::Ustr::from("__pace_promise_resolve"), prom_resolve_id);
    funcs.insert(ustr::Ustr::from("__pace_promise_await"), prom_await_id);
    funcs.insert(ustr::Ustr::from("__pace_is_err"), is_err_id);


    funcs
}
