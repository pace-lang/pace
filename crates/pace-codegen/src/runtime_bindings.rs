use cranelift_jit::JITBuilder;

pub struct RuntimeBindings;

impl RuntimeBindings {
    pub fn register_all(builder: &mut JITBuilder) {
        // Expose pace-runtime to the JIT explicitly
        builder.symbol(
            "__pace_print_int",
            pace_runtime::__pace_print_int as *const u8,
        );
        builder.symbol(
            "__pace_print_float",
            pace_runtime::__pace_print_float as *const u8,
        );
        builder.symbol(
            "__pace_print_string",
            pace_runtime::__pace_print_string as *const u8,
        );
        builder.symbol(
            "__pace_concat_strings",
            pace_runtime::__pace_concat_strings as *const u8,
        );
        builder.symbol(
            "__pace_int_to_string",
            pace_runtime::__pace_int_to_string as *const u8,
        );
        builder.symbol(
            "__pace_float_to_string",
            pace_runtime::__pace_float_to_string as *const u8,
        );
        builder.symbol(
            "__pace_bool_to_string",
            pace_runtime::__pace_bool_to_string as *const u8,
        );
        builder.symbol("__pace_malloc", pace_runtime::__pace_malloc as *const u8);
        builder.symbol("__pace_noop", pace_runtime::__pace_noop as *const u8);
        builder.symbol("__pace_retain", pace_runtime::__pace_retain as *const u8);
        builder.symbol("__pace_release", pace_runtime::__pace_release as *const u8);
        builder.symbol("__pace_free", pace_runtime::__pace_free as *const u8);
        builder.symbol(
            "__pace_ptr_store",
            pace_runtime::__pace_ptr_store as *const u8,
        );
        builder.symbol(
            "__pace_ptr_load",
            pace_runtime::__pace_ptr_load as *const u8,
        );
        builder.symbol("__pace_time", pace_runtime::__pace_time as *const u8);
        builder.symbol(
            "__pace_get_year",
            pace_runtime::__pace_get_year as *const u8,
        );
        builder.symbol("__pace_hash", pace_runtime::__pace_hash as *const u8);
        builder.symbol("__pace_sb_new", pace_runtime::__pace_sb_new as *const u8);
        builder.symbol(
            "__pace_sb_append",
            pace_runtime::__pace_sb_append as *const u8,
        );
        builder.symbol(
            "__pace_sb_build",
            pace_runtime::__pace_sb_build as *const u8,
        );
        builder.symbol("__pace_sb_free", pace_runtime::__pace_sb_free as *const u8);
        builder.symbol(
            "__pace_mailbox_create",
            pace_runtime::__pace_mailbox_create as *const u8,
        );
        builder.symbol(
            "__pace_mailbox_send",
            pace_runtime::__pace_mailbox_send as *const u8,
        );
        builder.symbol(
            "__pace_mailbox_destroy",
            pace_runtime::__pace_mailbox_destroy as *const u8,
        );
        builder.symbol(
            "__pace_promise_create",
            pace_runtime::__pace_promise_create as *const u8,
        );
        builder.symbol(
            "__pace_promise_resolve",
            pace_runtime::__pace_promise_resolve as *const u8,
        );
        builder.symbol(
            "__pace_promise_await",
            pace_runtime::__pace_promise_await as *const u8,
        );

        // Add FS, OS, Process, HTTP, and String symbols
        builder.symbol("fsWriteText", pace_runtime::__pace_fs_write as *const u8);
        builder.symbol("fsExists", pace_runtime::__pace_fs_exists as *const u8);
        builder.symbol("fsReadText", pace_runtime::__pace_fs_read as *const u8);
        builder.symbol("fsDeleteFile", pace_runtime::__pace_fs_delete as *const u8);
        builder.symbol("fsMakeDir", pace_runtime::__pace_fs_mkdir as *const u8);
        builder.symbol(
            "fsDirExists",
            pace_runtime::__pace_fs_dir_exists as *const u8,
        );

        builder.symbol("osGetEnv", pace_runtime::__pace_os_getenv as *const u8);
        builder.symbol("osName", pace_runtime::__pace_os_name as *const u8);

        builder.symbol("processRun", pace_runtime::__pace_process_run as *const u8);
        builder.symbol(
            "processExit",
            pace_runtime::__pace_process_exit as *const u8,
        );

        builder.symbol("httpGet", pace_runtime::__pace_http_get as *const u8);
        builder.symbol("httpPost", pace_runtime::__pace_http_post as *const u8);
        builder.symbol("httpPut", pace_runtime::__pace_http_put as *const u8);
        builder.symbol("httpDelete", pace_runtime::__pace_http_delete as *const u8);

        builder.symbol(
            "getLastError",
            pace_runtime::__pace_get_last_error as *const u8,
        );
        builder.symbol(
            "__pace_string_split",
            pace_runtime::__pace_string_split as *const u8,
        );
        builder.symbol(
            "__pace_string_replace",
            pace_runtime::__pace_string_replace as *const u8,
        );
        builder.symbol(
            "__pace_string_substring",
            pace_runtime::__pace_string_substring as *const u8,
        );
        builder.symbol("__pace_string_trim", pace_runtime::__pace_string_trim as *const u8);
        builder.symbol(
            "__pace_string_index_of",
            pace_runtime::__pace_string_index_of as *const u8,
        );
        builder.symbol(
            "__pace_string_starts_with",
            pace_runtime::__pace_string_starts_with as *const u8,
        );
    }
}
