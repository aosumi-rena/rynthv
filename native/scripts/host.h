#ifndef RYNTHV_SCRIPT_HOST_H
#define RYNTHV_SCRIPT_HOST_H
#include "quickjs/quickjs.h"
#include "lua/lua.h"
typedef const char *(*request_fn)(void *,const char *);
typedef int (*cancel_fn)(void *);
typedef struct {
    JSRuntime *runtime;
    JSContext *context;
    request_fn request;
    cancel_fn cancel;
    void *opaque;
    char *result;
    lua_State *lua;
    JSClassID lua_callback_class;
    size_t lua_bytes;
} ScriptHost;
int script_result(ScriptHost *,JSValue);
int script_lua_load(ScriptHost *,const char *,size_t,const char *);
void script_lua_destroy(ScriptHost *);
#endif
