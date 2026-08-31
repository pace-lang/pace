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

    let mut sig_ptr_load = module.make_signature();
    sig_ptr_load.params.push(AbiParam::new(ptr_ty));
    sig_ptr_load.params.push(AbiParam::new(types::I64));
    sig_ptr_load.returns.push(AbiParam::new(types::I64));
    let ptr_load_id = module
        .declare_function("__pace_ptr_load", Linkage::Import, &sig_ptr_load)
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

    let mut sig_sb_new = module.make_signature();
    sig_sb_new.returns.push(AbiParam::new(ptr_ty));
    let sb_new_id = module
        .declare_function("__pace_sb_new", Linkage::Import, &sig_sb_new)
        .unwrap();

    let mut sig_sb_append = module.make_signature();
    sig_sb_append.params.push(AbiParam::new(ptr_ty));
    sig_sb_append.params.push(AbiParam::new(ptr_ty));
    let sb_append_id = module
        .declare_function("__pace_sb_append", Linkage::Import, &sig_sb_append)
        .unwrap();

    let mut sig_sb_build = module.make_signature();
    sig_sb_build.params.push(AbiParam::new(ptr_ty));
    sig_sb_build.returns.push(AbiParam::new(ptr_ty));
    let sb_build_id = module
        .declare_function("__pace_sb_build", Linkage::Import, &sig_sb_build)
        .unwrap();

    let mut sig_sb_free = module.make_signature();
    sig_sb_free.params.push(AbiParam::new(ptr_ty));
    let sb_free_id = module
        .declare_function("__pace_sb_free", Linkage::Import, &sig_sb_free)
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
    funcs.insert(ustr::Ustr::from("ptrLoad"), ptr_load_id);
    funcs.insert(ustr::Ustr::from("time"), time_id);
    funcs.insert(ustr::Ustr::from("getYear"), get_year_id);
    funcs.insert(ustr::Ustr::from("hash"), hash_id);
    funcs.insert(ustr::Ustr::from("sbNew"), sb_new_id);
    funcs.insert(ustr::Ustr::from("sbAppend"), sb_append_id);
    funcs.insert(ustr::Ustr::from("sbBuild"), sb_build_id);
    funcs.insert(ustr::Ustr::from("sbFree"), sb_free_id);
    funcs.insert(ustr::Ustr::from("__pace_mailbox_create"), mb_create_id);
    funcs.insert(ustr::Ustr::from("__pace_mailbox_send"), mb_send_id);
    funcs.insert(ustr::Ustr::from("__pace_mailbox_destroy"), mb_destroy_id);
    funcs.insert(ustr::Ustr::from("__pace_promise_create"), prom_create_id);
    funcs.insert(ustr::Ustr::from("__pace_promise_resolve"), prom_resolve_id);
    funcs.insert(ustr::Ustr::from("__pace_promise_await"), prom_await_id);

    let mut sig_fs_write_text = module.make_signature();
    sig_fs_write_text.params.push(AbiParam::new(ptr_ty));
    sig_fs_write_text.params.push(AbiParam::new(ptr_ty));
    sig_fs_write_text.returns.push(AbiParam::new(types::I64));
    let fs_write_text_id = module
        .declare_function("fsWriteText", Linkage::Import, &sig_fs_write_text)
        .unwrap();
    funcs.insert(ustr::Ustr::from("fsWriteText"), fs_write_text_id);

    let mut sig_fs_exists = module.make_signature();
    sig_fs_exists.params.push(AbiParam::new(ptr_ty));
    sig_fs_exists.returns.push(AbiParam::new(types::I64));
    let fs_exists_id = module
        .declare_function("fsExists", Linkage::Import, &sig_fs_exists)
        .unwrap();
    funcs.insert(ustr::Ustr::from("fsExists"), fs_exists_id);

    let mut sig_fs_read_text = module.make_signature();
    sig_fs_read_text.params.push(AbiParam::new(ptr_ty));
    sig_fs_read_text.returns.push(AbiParam::new(ptr_ty));
    let fs_read_text_id = module
        .declare_function("fsReadText", Linkage::Import, &sig_fs_read_text)
        .unwrap();
    funcs.insert(ustr::Ustr::from("fsReadText"), fs_read_text_id);

    let mut sig_fs_delete_file = module.make_signature();
    sig_fs_delete_file.params.push(AbiParam::new(ptr_ty));
    sig_fs_delete_file.returns.push(AbiParam::new(types::I64));
    let fs_delete_file_id = module
        .declare_function("fsDeleteFile", Linkage::Import, &sig_fs_delete_file)
        .unwrap();
    funcs.insert(ustr::Ustr::from("fsDeleteFile"), fs_delete_file_id);

    let mut sig_fs_mkdir = module.make_signature();
    sig_fs_mkdir.params.push(AbiParam::new(ptr_ty));
    sig_fs_mkdir.returns.push(AbiParam::new(types::I64));
    let fs_mkdir_id = module
        .declare_function("fsMakeDir", Linkage::Import, &sig_fs_mkdir)
        .unwrap();
    funcs.insert(ustr::Ustr::from("fsMakeDir"), fs_mkdir_id);

    let mut sig_fs_dir_exists = module.make_signature();
    sig_fs_dir_exists.params.push(AbiParam::new(ptr_ty));
    sig_fs_dir_exists.returns.push(AbiParam::new(types::I64));
    let fs_dir_exists_id = module
        .declare_function("fsDirExists", Linkage::Import, &sig_fs_dir_exists)
        .unwrap();
    funcs.insert(ustr::Ustr::from("fsDirExists"), fs_dir_exists_id);

    let mut sig_os_getenv = module.make_signature();
    sig_os_getenv.params.push(AbiParam::new(ptr_ty));
    sig_os_getenv.returns.push(AbiParam::new(ptr_ty));
    let os_getenv_id = module
        .declare_function("osGetEnv", Linkage::Import, &sig_os_getenv)
        .unwrap();
    funcs.insert(ustr::Ustr::from("osGetEnv"), os_getenv_id);

    let mut sig_os_name = module.make_signature();
    sig_os_name.returns.push(AbiParam::new(ptr_ty));
    let os_name_id = module
        .declare_function("osName", Linkage::Import, &sig_os_name)
        .unwrap();
    funcs.insert(ustr::Ustr::from("osName"), os_name_id);

    let mut sig_process_run = module.make_signature();
    sig_process_run.params.push(AbiParam::new(ptr_ty));
    sig_process_run.returns.push(AbiParam::new(ptr_ty));
    let process_run_id = module
        .declare_function("processRun", Linkage::Import, &sig_process_run)
        .unwrap();
    funcs.insert(ustr::Ustr::from("processRun"), process_run_id);

    let mut sig_process_exit = module.make_signature();
    sig_process_exit.params.push(AbiParam::new(types::I64));
    let process_exit_id = module
        .declare_function("processExit", Linkage::Import, &sig_process_exit)
        .unwrap();
    funcs.insert(ustr::Ustr::from("processExit"), process_exit_id);

    let mut sig_http_get = module.make_signature();
    sig_http_get.params.push(AbiParam::new(ptr_ty));
    sig_http_get.returns.push(AbiParam::new(ptr_ty));
    let http_get_id = module
        .declare_function("httpGet", Linkage::Import, &sig_http_get)
        .unwrap();
    funcs.insert(ustr::Ustr::from("httpGet"), http_get_id);

    let mut sig_http_post = module.make_signature();
    sig_http_post.params.push(AbiParam::new(ptr_ty));
    sig_http_post.params.push(AbiParam::new(ptr_ty));
    sig_http_post.returns.push(AbiParam::new(ptr_ty));
    let http_post_id = module
        .declare_function("httpPost", Linkage::Import, &sig_http_post)
        .unwrap();
    funcs.insert(ustr::Ustr::from("httpPost"), http_post_id);

    let mut sig_http_put = module.make_signature();
    sig_http_put.params.push(AbiParam::new(ptr_ty));
    sig_http_put.params.push(AbiParam::new(ptr_ty));
    sig_http_put.returns.push(AbiParam::new(ptr_ty));
    let http_put_id = module
        .declare_function("httpPut", Linkage::Import, &sig_http_put)
        .unwrap();
    funcs.insert(ustr::Ustr::from("httpPut"), http_put_id);

    let mut sig_http_delete = module.make_signature();
    sig_http_delete.params.push(AbiParam::new(ptr_ty));
    sig_http_delete.returns.push(AbiParam::new(ptr_ty));
    let http_delete_id = module
        .declare_function("httpDelete", Linkage::Import, &sig_http_delete)
        .unwrap();
    funcs.insert(ustr::Ustr::from("httpDelete"), http_delete_id);

    let mut sig_get_last_error = module.make_signature();
    sig_get_last_error.returns.push(AbiParam::new(ptr_ty));
    let get_last_error_id = module
        .declare_function("getLastError", Linkage::Import, &sig_get_last_error)
        .unwrap();
    funcs.insert(ustr::Ustr::from("getLastError"), get_last_error_id);

    let mut sig_string_split = module.make_signature();
    sig_string_split.params.push(AbiParam::new(ptr_ty));
    sig_string_split.params.push(AbiParam::new(ptr_ty));
    sig_string_split.returns.push(AbiParam::new(types::I64));
    let string_split_id = module
        .declare_function("__pace_string_split", Linkage::Import, &sig_string_split)
        .unwrap();
    funcs.insert(ustr::Ustr::from("stringSplit"), string_split_id);

    let mut sig_string_replace = module.make_signature();
    sig_string_replace.params.push(AbiParam::new(ptr_ty));
    sig_string_replace.params.push(AbiParam::new(ptr_ty));
    sig_string_replace.params.push(AbiParam::new(ptr_ty));
    sig_string_replace.returns.push(AbiParam::new(ptr_ty));
    let string_replace_id = module
        .declare_function(
            "__pace_string_replace",
            Linkage::Import,
            &sig_string_replace,
        )
        .unwrap();
    funcs.insert(ustr::Ustr::from("stringReplace"), string_replace_id);

    let mut sig_string_substring = module.make_signature();
    sig_string_substring.params.push(AbiParam::new(ptr_ty));
    sig_string_substring.params.push(AbiParam::new(types::I64));
    sig_string_substring.params.push(AbiParam::new(types::I64));
    sig_string_substring.returns.push(AbiParam::new(ptr_ty));
    let string_substring_id = module
        .declare_function(
            "__pace_string_substring",
            Linkage::Import,
            &sig_string_substring,
        )
        .unwrap();
    funcs.insert(ustr::Ustr::from("stringSubstring"), string_substring_id);

    let mut sig_string_trim = module.make_signature();
    sig_string_trim.params.push(AbiParam::new(ptr_ty));
    sig_string_trim.returns.push(AbiParam::new(ptr_ty));
    let string_trim_id = module
        .declare_function("__pace_string_trim", Linkage::Import, &sig_string_trim)
        .unwrap();
    funcs.insert(ustr::Ustr::from("stringTrim"), string_trim_id);

    let mut sig_string_index_of = module.make_signature();
    sig_string_index_of.params.push(AbiParam::new(ptr_ty));
    sig_string_index_of.params.push(AbiParam::new(ptr_ty));
    sig_string_index_of.returns.push(AbiParam::new(types::I64));
    let string_index_of_id = module
        .declare_function(
            "__pace_string_index_of",
            Linkage::Import,
            &sig_string_index_of,
        )
        .unwrap();
    funcs.insert(ustr::Ustr::from("stringIndexOf"), string_index_of_id);

    let mut sig_string_starts_with = module.make_signature();
    sig_string_starts_with.params.push(AbiParam::new(ptr_ty));
    sig_string_starts_with.params.push(AbiParam::new(ptr_ty));
    sig_string_starts_with
        .returns
        .push(AbiParam::new(types::I64));
    let string_starts_with_id = module
        .declare_function(
            "__pace_string_starts_with",
            Linkage::Import,
            &sig_string_starts_with,
        )
        .unwrap();
    funcs.insert(ustr::Ustr::from("stringStartsWith"), string_starts_with_id);
    funcs
}
