//! Handle-based method dispatch for perry-stdlib
//!
//! When native modules (Fastify, ioredis, etc.) use handle-based objects,
//! and those handles are passed to functions as generic parameters,
//! the codegen can't statically determine the type. This module provides
//! runtime dispatch by checking the handle type in the registry.

use super::handle::*;

/// Dispatch a method call on a handle-based object.
/// Called from perry-runtime's js_native_call_method when it detects a handle
/// (pointer value < 0x100000, indicating an integer handle, not a real heap pointer).
#[no_mangle]
pub unsafe extern "C" fn js_handle_method_dispatch(
    handle: i64,
    method_name_ptr: *const u8,
    method_name_len: usize,
    args_ptr: *const f64,
    args_len: usize,
) -> f64 {
    let method_name = if method_name_ptr.is_null() || method_name_len == 0 {
        ""
    } else {
        std::str::from_utf8(std::slice::from_raw_parts(method_name_ptr, method_name_len))
            .unwrap_or("")
    };
    let args: &[f64] = if args_len > 0 && !args_ptr.is_null() {
        std::slice::from_raw_parts(args_ptr, args_len)
    } else {
        &[]
    };
    // `_` prefixes silence unused-variable warnings when every dispatch
    // arm below is compiled out (e.g. minimal-stdlib without http-server
    // / database-redis).
    let _ = method_name;
    let _ = args;
    let _ = handle;

    // Try Fastify app dispatch
    #[cfg(feature = "http-server")]
    if with_handle::<crate::fastify::FastifyApp, bool, _>(handle, |_| true).unwrap_or(false) {
        return dispatch_fastify_app(handle, method_name, args);
    }

    // Try Fastify context dispatch (request/reply)
    #[cfg(feature = "http-server")]
    if with_handle::<crate::fastify::FastifyContext, bool, _>(handle, |_| true).unwrap_or(false) {
        return dispatch_fastify_context(handle, method_name, args);
    }

    // Try ioredis Redis client dispatch
    #[cfg(feature = "database-redis")]
    if with_handle::<crate::ioredis::RedisClient, bool, _>(handle, |_| true).unwrap_or(false) {
        return dispatch_ioredis(handle, method_name, args);
    }

    // Try net.Socket dispatch — covers cases where the socket handle is
    // accessed through a wrapper function, struct field, or Map.get and
    // the codegen lost type info. The static NATIVE_MODULE_TABLE path is
    // still preferred when types are visible (zero-overhead direct call),
    // but this fallback unblocks library code that wraps net.createConnection.
    #[cfg(all(feature = "net", not(target_os = "ios"), not(target_os = "android")))]
    if crate::net::is_net_socket_handle(handle) {
        return dispatch_net_socket(handle, method_name, args);
    }

    // Try container-compose dispatch
    #[cfg(feature = "container")]
    if with_handle::<perry_container_compose::types::ComposeHandle, bool, _>(handle, |_| true).unwrap_or(false) {
        return dispatch_container_compose(handle, method_name, args);
    }

    // Unknown handle type - return undefined
    f64::from_bits(0x7FF8_0000_0000_0001)
}

/// Dispatch method calls on Fastify app handles
#[cfg(feature = "http-server")]
unsafe fn dispatch_fastify_app(handle: i64, method: &str, args: &[f64]) -> f64 {
    match method {
        "get" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            // Support 3-arg form: fastify.get(path, options, handler) — skip options object
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_get(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "post" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_post(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "put" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_put(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "delete" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_delete(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "patch" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_patch(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "head" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_head(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "options" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_options(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "all" if args.len() >= 2 => {
            let path = args[0].to_bits() as i64;
            let handler = if args.len() >= 3 { args[2].to_bits() as i64 } else { args[1].to_bits() as i64 };
            let result = crate::fastify::js_fastify_all(handle, path, handler);
            if result { 1.0 } else { 0.0 }
        }
        "addHook" if args.len() >= 2 => {
            let hook_name = args[0].to_bits() as i64;
            let handler = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_add_hook(handle, hook_name, handler);
            if result { 1.0 } else { 0.0 }
        }
        "setErrorHandler" if args.len() >= 1 => {
            let handler = args[0].to_bits() as i64;
            let result = crate::fastify::js_fastify_set_error_handler(handle, handler);
            if result { 1.0 } else { 0.0 }
        }
        "register" if args.len() >= 1 => {
            let plugin = args[0].to_bits() as i64;
            let opts = if args.len() >= 2 { args[1] } else { f64::from_bits(0x7FF8_0000_0000_0001) };
            let result = crate::fastify::js_fastify_register(handle, plugin, opts);
            if result { 1.0 } else { 0.0 }
        }
        "listen" if args.len() >= 1 => {
            let callback = if args.len() >= 2 { args[1].to_bits() as i64 } else { 0 };
            crate::fastify::js_fastify_listen(handle, args[0], callback);
            f64::from_bits(0x7FF8_0000_0000_0001) // undefined (void)
        }
        _ => {
            // Unknown method - return undefined
            f64::from_bits(0x7FF8_0000_0000_0001)
        }
    }
}

/// Dispatch method calls on Fastify context handles (request/reply)
#[cfg(feature = "http-server")]
unsafe fn dispatch_fastify_context(handle: i64, method: &str, args: &[f64]) -> f64 {
    use perry_runtime::JSValue;

    match method {
        // Reply methods
        "send" if args.len() >= 1 => {
            let result = crate::fastify::js_fastify_reply_send(handle, args[0]);
            if result { 1.0 } else { 0.0 }
        }
        "status" | "code" if args.len() >= 1 => {
            let result = crate::fastify::js_fastify_reply_status(handle, args[0]);
            // Return the handle as NaN-boxed pointer for chaining (reply.status(200).send(...))
            f64::from_bits(0x7FFD_0000_0000_0000 | (result as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        "header" if args.len() >= 2 => {
            let name = args[0].to_bits() as i64;
            let value = args[1].to_bits() as i64;
            let result = crate::fastify::js_fastify_reply_header(handle, name, value);
            // Return the handle for chaining
            f64::from_bits(0x7FFD_0000_0000_0000 | (result as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        // Request methods
        "method" => {
            let ptr = crate::fastify::js_fastify_req_method(handle);
            f64::from_bits(JSValue::string_ptr(ptr).bits())
        }
        "url" => {
            let ptr = crate::fastify::js_fastify_req_url(handle);
            f64::from_bits(JSValue::string_ptr(ptr).bits())
        }
        "body" => {
            crate::fastify::js_fastify_req_json(handle)
        }
        "json" => {
            crate::fastify::js_fastify_req_json(handle)
        }
        "params" => {
            crate::fastify::js_fastify_req_params_object(handle)
        }
        "headers" => {
            // Returns NaN-boxed JS object (parsed from JSON), use bits directly
            let bits = crate::fastify::js_fastify_req_headers(handle);
            f64::from_bits(bits as u64)
        }
        _ => {
            // Unknown method - return undefined
            f64::from_bits(0x7FF8_0000_0000_0001)
        }
    }
}

/// Dispatch method calls on net.Socket handles when codegen couldn't tag
/// the receiver type. Mirrors the static NATIVE_MODULE_TABLE entries for
/// the same methods (write/end/destroy/on/upgradeToTLS).
///
/// Args arrive as NaN-boxed `f64`s: BufferHeader / StringHeader / Closure
/// pointers in the low 48 bits with POINTER_TAG / STRING_TAG in the top.
/// We strip the tag and pass the raw `i64` to the FFI — same shape the
/// codegen path produces.
#[cfg(all(feature = "net", not(target_os = "ios"), not(target_os = "android")))]
unsafe fn dispatch_net_socket(handle: i64, method: &str, args: &[f64]) -> f64 {
    /// Strip a NaN-box tag (POINTER / STRING / BIGINT) to get the raw 48-bit pointer.
    fn unbox_to_i64(v: f64) -> i64 {
        (v.to_bits() & 0x0000_FFFF_FFFF_FFFF) as i64
    }

    match method {
        "write" if !args.is_empty() => {
            crate::net::js_net_socket_write(handle, unbox_to_i64(args[0]));
            f64::from_bits(0x7FFC_0000_0000_0001) // undefined
        }
        "end" => {
            crate::net::js_net_socket_end(handle);
            f64::from_bits(0x7FFC_0000_0000_0001)
        }
        "destroy" => {
            crate::net::js_net_socket_destroy(handle);
            f64::from_bits(0x7FFC_0000_0000_0001)
        }
        "on" if args.len() >= 2 => {
            let event_ptr = unbox_to_i64(args[0]);
            let cb_ptr = unbox_to_i64(args[1]);
            crate::net::js_net_socket_on(handle, event_ptr, cb_ptr);
            f64::from_bits(0x7FFC_0000_0000_0001)
        }
        "upgradeToTLS" if !args.is_empty() => {
            // upgradeToTLS(servername, verify) → Promise. Default verify=1
            // when omitted, mirroring the safer default in the static table.
            let servername_ptr = unbox_to_i64(args[0]);
            let verify = if args.len() >= 2 { args[1] } else { 1.0 };
            let promise = crate::net::js_net_socket_upgrade_tls(handle, servername_ptr, verify);
            f64::from_bits(0x7FFD_0000_0000_0000u64 | (promise as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        _ => {
            f64::from_bits(0x7FFC_0000_0000_0001)
        }
    }
}

/// Dispatch a property access on a handle-based object.
/// Called from perry-runtime's js_dynamic_object_get_property when it detects a handle.
#[no_mangle]
pub unsafe extern "C" fn js_handle_property_dispatch(
    handle: i64,
    property_name_ptr: *const u8,
    property_name_len: usize,
) -> f64 {
    #[cfg(feature = "http-server")]
    use perry_runtime::JSValue;

    let property_name = if property_name_ptr.is_null() || property_name_len == 0 {
        ""
    } else {
        std::str::from_utf8(std::slice::from_raw_parts(property_name_ptr, property_name_len))
            .unwrap_or("")
    };
    let _ = property_name;
    let _ = handle;

    // Try Fastify context dispatch (request/reply properties)
    #[cfg(feature = "http-server")]
    if with_handle::<crate::fastify::FastifyContext, bool, _>(handle, |_| true).unwrap_or(false) {
        return match property_name {
            "query" => {
                // Return a real JavaScript object, not a JSON string
                crate::fastify::js_fastify_req_query_object(handle)
            }
            "params" => {
                crate::fastify::js_fastify_req_params_object(handle)
            }
            "body" => {
                crate::fastify::js_fastify_req_json(handle)
            }
            "rawBody" | "text" => {
                let ptr = crate::fastify::js_fastify_req_body(handle);
                if ptr.is_null() {
                    f64::from_bits(0x7FFC_0000_0000_0001)
                } else {
                    f64::from_bits(JSValue::string_ptr(ptr).bits())
                }
            }
            "headers" => {
                // Returns NaN-boxed JS object (parsed from JSON), use bits directly
                let bits = crate::fastify::js_fastify_req_headers(handle);
                f64::from_bits(bits as u64)
            }
            "method" => {
                let ptr = crate::fastify::js_fastify_req_method(handle);
                if ptr.is_null() {
                    f64::from_bits(0x7FFC_0000_0000_0001)
                } else {
                    f64::from_bits(JSValue::string_ptr(ptr).bits())
                }
            }
            "url" => {
                let ptr = crate::fastify::js_fastify_req_url(handle);
                if ptr.is_null() {
                    f64::from_bits(0x7FFC_0000_0000_0001)
                } else {
                    f64::from_bits(JSValue::string_ptr(ptr).bits())
                }
            }
            "user" => {
                // Return user data set by auth middleware
                crate::fastify::js_fastify_req_get_user_data(handle)
            }
            _ => f64::from_bits(0x7FFC_0000_0000_0001), // undefined
        };
    }

    // Unknown handle type - return undefined
    f64::from_bits(0x7FFC_0000_0000_0001)
}

/// Dispatch method calls on ioredis Redis client handles
#[cfg(feature = "database-redis")]
unsafe fn dispatch_ioredis(handle: i64, method: &str, args: &[f64]) -> f64 {
    // Helper: extract raw StringHeader pointer from NaN-boxed f64
    fn get_string_ptr(val: f64) -> *const perry_runtime::StringHeader {
        let bits = val.to_bits();
        // Strip STRING_TAG (0x7FFF) to get raw pointer
        (bits & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::StringHeader
    }

    // Helper: NaN-box a Promise pointer with POINTER_TAG for return
    fn nanbox_promise(promise: *mut perry_runtime::Promise) -> f64 {
        let bits = (promise as u64) | 0x7FFD_0000_0000_0000;
        f64::from_bits(bits)
    }

    match method {
        "connect" => {
            let promise = crate::ioredis::js_ioredis_connect(handle);
            nanbox_promise(promise)
        }
        "get" if !args.is_empty() => {
            let key_ptr = get_string_ptr(args[0]);
            let promise = crate::ioredis::js_ioredis_get(handle, key_ptr);
            nanbox_promise(promise)
        }
        "set" if args.len() >= 2 => {
            let key_ptr = get_string_ptr(args[0]);
            let value_ptr = get_string_ptr(args[1]);
            let promise = crate::ioredis::js_ioredis_set(handle, key_ptr, value_ptr);
            nanbox_promise(promise)
        }
        "setex" if args.len() >= 3 => {
            let key_ptr = get_string_ptr(args[0]);
            let seconds = args[1];
            let value_ptr = get_string_ptr(args[2]);
            let promise = crate::ioredis::js_ioredis_setex(handle, key_ptr, seconds, value_ptr);
            nanbox_promise(promise)
        }
        "del" if !args.is_empty() => {
            let key_ptr = get_string_ptr(args[0]);
            let promise = crate::ioredis::js_ioredis_del(handle, key_ptr);
            nanbox_promise(promise)
        }
        "exists" if !args.is_empty() => {
            let key_ptr = get_string_ptr(args[0]);
            let promise = crate::ioredis::js_ioredis_exists(handle, key_ptr);
            nanbox_promise(promise)
        }
        "incr" if !args.is_empty() => {
            let key_ptr = get_string_ptr(args[0]);
            let promise = crate::ioredis::js_ioredis_incr(handle, key_ptr);
            nanbox_promise(promise)
        }
        "decr" if !args.is_empty() => {
            let key_ptr = get_string_ptr(args[0]);
            let promise = crate::ioredis::js_ioredis_decr(handle, key_ptr);
            nanbox_promise(promise)
        }
        "expire" if args.len() >= 2 => {
            let key_ptr = get_string_ptr(args[0]);
            let seconds = args[1];
            let promise = crate::ioredis::js_ioredis_expire(handle, key_ptr, seconds);
            nanbox_promise(promise)
        }
        "ping" => {
            let promise = crate::ioredis::js_ioredis_ping(handle);
            nanbox_promise(promise)
        }
        "quit" => {
            let promise = crate::ioredis::js_ioredis_quit(handle);
            nanbox_promise(promise)
        }
        "disconnect" => {
            crate::ioredis::js_ioredis_disconnect(handle);
            f64::from_bits(0x7FFC_0000_0000_0001) // undefined
        }
        _ => {
            f64::from_bits(0x7FFC_0000_0000_0001) // undefined
        }
    }
}

/// Dispatch property set on a handle-based object.
/// Called from perry-runtime's js_object_set_field_by_name when it detects a handle.
#[no_mangle]
pub unsafe extern "C" fn js_handle_property_set_dispatch(
    handle: i64,
    property_name_ptr: *const u8,
    property_name_len: usize,
    value: f64,
) {
    let property_name = if property_name_ptr.is_null() || property_name_len == 0 {
        ""
    } else {
        std::str::from_utf8(std::slice::from_raw_parts(property_name_ptr, property_name_len))
            .unwrap_or("")
    };
    let _ = property_name;
    let _ = handle;
    let _ = value;

    // Try Fastify context dispatch (request/reply properties)
    #[cfg(feature = "http-server")]
    if with_handle::<crate::fastify::FastifyContext, bool, _>(handle, |_| true).unwrap_or(false) {
        match property_name {
            "user" => {
                crate::fastify::js_fastify_req_set_user_data(handle, value);
            }
            _ => {}
        }
    }
}

/// Initialize the handle method and property dispatch systems.
/// This registers our dispatch functions with perry-runtime.
/// Must be called before any user code runs.
#[no_mangle]
pub unsafe extern "C" fn js_stdlib_init_dispatch() {
    extern "C" {
        fn js_register_handle_method_dispatch(
            f: unsafe extern "C" fn(i64, *const u8, usize, *const f64, usize) -> f64,
        );
        fn js_register_handle_property_dispatch(
            f: unsafe extern "C" fn(i64, *const u8, usize) -> f64,
        );
        fn js_register_handle_property_set_dispatch(
            f: unsafe extern "C" fn(i64, *const u8, usize, f64),
        );
    }
    js_register_handle_method_dispatch(js_handle_method_dispatch);
    js_register_handle_property_dispatch(js_handle_property_dispatch);
    js_register_handle_property_set_dispatch(js_handle_property_set_dispatch);
}

/// Dispatch method calls on container-compose handles
#[cfg(feature = "container")]
unsafe fn dispatch_container_compose(handle: i64, method: &str, args: &[f64]) -> f64 {
    use perry_runtime::value::js_is_truthy;
    use perry_runtime::object::js_object_get_field_by_name_f64;
    use perry_runtime::array::{js_array_length, js_array_get_f64};

    match method {
        "down" => {
            let options = if args.len() >= 1 { args[0] } else { f64::from_bits(0x7FFC_0000_0000_0001) };
            let volumes = if !options.is_nan() && (options.to_bits() & 0x7FFF_0000_0000_0000) == 0x7FFD_0000_0000_0000 {
                 let obj_ptr = (options.to_bits() & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::ObjectHeader;
                 let key_str = "volumes";
                 let key_ptr = perry_runtime::js_string_from_bytes(key_str.as_ptr(), key_str.len() as u32);
                 let volumes_val = js_object_get_field_by_name_f64(obj_ptr, key_ptr);
                 js_is_truthy(volumes_val) != 0
            } else {
                false
            };
            let promise = crate::container::js_container_compose_down(handle, if volumes { 1 } else { 0 });
            f64::from_bits(0x7FFD_0000_0000_0000u64 | (promise as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        "ps" => {
            let promise = crate::container::js_container_compose_ps(handle);
            f64::from_bits(0x7FFD_0000_0000_0000u64 | (promise as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        "logs" => {
            let options = if args.len() >= 1 { args[0] } else { f64::from_bits(0x7FFC_0000_0000_0001) };
            let mut service_ptr = std::ptr::null();
            let mut tail = -1;

            if !options.is_nan() && (options.to_bits() & 0x7FFF_0000_0000_0000) == 0x7FFD_0000_0000_0000 {
                 let obj_ptr = (options.to_bits() & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::ObjectHeader;

                 // Extract service
                 let s_key_str = "service";
                 let s_key_ptr = perry_runtime::js_string_from_bytes(s_key_str.as_ptr(), s_key_str.len() as u32);
                 let s_val = js_object_get_field_by_name_f64(obj_ptr, s_key_ptr);
                 if (s_val.to_bits() & 0x7FFF_0000_0000_0000) == 0x7FFF_0000_0000_0000 {
                     service_ptr = (s_val.to_bits() & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::StringHeader;
                 }

                 // Extract tail
                 let t_key_str = "tail";
                 let t_key_ptr = perry_runtime::js_string_from_bytes(t_key_str.as_ptr(), t_key_str.len() as u32);
                 let t_val = js_object_get_field_by_name_f64(obj_ptr, t_key_ptr);
                 if !t_val.is_nan() && (t_val.to_bits() & 0x7FFE_0000_0000_0000) != 0x7FF8_0000_0000_0000 {
                     tail = t_val as i32;
                 }
            }
            let promise = crate::container::js_container_compose_logs(handle, service_ptr, tail);
            f64::from_bits(0x7FFD_0000_0000_0000u64 | (promise as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        "exec" if args.len() >= 2 => {
            let s_val = args[0];
            if (s_val.to_bits() & 0x7FFF_0000_0000_0000) != 0x7FFF_0000_0000_0000 {
                return f64::from_bits(0x7FFC_0000_0000_0001); // return undefined
            }
            let service_ptr = (s_val.to_bits() & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::StringHeader;

            let c_val = args[1];
            if (c_val.to_bits() & 0x7FFF_0000_0000_0000) != 0x7FFD_0000_0000_0000 {
                 return f64::from_bits(0x7FFC_0000_0000_0001);
            }
            let cmd_ptr = (c_val.to_bits() & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::ArrayHeader;

            let len = js_array_length(cmd_ptr);
            let mut cmd_vec = Vec::new();
            for i in 0..len {
                let val = js_array_get_f64(cmd_ptr, i);
                // Verify it's a string
                if (val.to_bits() & 0x7FFF_0000_0000_0000) == 0x7FFF_0000_0000_0000 {
                    let s_ptr = (val.to_bits() & 0x0000_FFFF_FFFF_FFFF) as *const perry_runtime::StringHeader;
                    if !s_ptr.is_null() {
                        let s_len = (*s_ptr).byte_len as usize;
                        let s_data = (s_ptr as *const u8).add(std::mem::size_of::<perry_runtime::StringHeader>());
                        let s = std::str::from_utf8(std::slice::from_raw_parts(s_data, s_len)).unwrap_or("");
                        cmd_vec.push(s.to_string());
                    }
                }
            }
            let cmd_json = serde_json::to_string(&cmd_vec).unwrap_or_else(|_| "[]".to_string());
            let cmd_json_ptr = perry_runtime::js_string_from_bytes(cmd_json.as_ptr(), cmd_json.len() as u32);
            let promise = crate::container::js_container_compose_exec(handle, service_ptr, cmd_json_ptr);
            f64::from_bits(0x7FFD_0000_0000_0000u64 | (promise as u64 & 0x0000_FFFF_FFFF_FFFF))
        }
        _ => f64::from_bits(0x7FFC_0000_0000_0001), // undefined
    }
}
