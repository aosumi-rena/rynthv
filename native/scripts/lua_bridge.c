#include "host.h"
#include "lua/lauxlib.h"
#include "lua/lualib.h"
#include <stdlib.h>
#include <string.h>
#define OBJECT "RynthV.ScriptObject"
typedef struct {ScriptHost *host;JSValue value;} Object;
typedef struct {ScriptHost *host;int ref;} Callback;
static char object_map,object_cache;
static Object *lookup(lua_State *L,int index) {
    Object *o=luaL_testudata(L,index,OBJECT);if(o)return o;
    if(!lua_istable(L,index))return NULL;index=lua_absindex(L,index);
    lua_rawgetp(L,LUA_REGISTRYINDEX,&object_map);lua_pushvalue(L,index);lua_rawget(L,-2);
    o=luaL_testudata(L,-1,OBJECT);lua_pop(L,2);return o;
}
static JSValue to_js(ScriptHost *,int,int);
static int to_lua(ScriptHost *,JSValueConst,int);
static void callback_free(JSRuntime *rt,JSValue value) {
    (void)rt;Callback *callback=JS_GetOpaque(value,JS_GetClassID(value));
    if(callback){if(callback->host->lua)luaL_unref(callback->host->lua,LUA_REGISTRYINDEX,callback->ref);free(callback);}
}
static void *lua_allocate(void *opaque,void *pointer,size_t old,size_t size) {
    ScriptHost *h=opaque;if(!pointer)old=0;
    if(!size){free(pointer);h->lua_bytes-=old;return NULL;}
    if(size>256u*1024u*1024u||h->lua_bytes-old>256u*1024u*1024u-size)return NULL;
    void *next=realloc(pointer,size);if(next)h->lua_bytes=h->lua_bytes-old+size;return next;
}
static JSValue helper(ScriptHost *h,const char *name,int n,JSValueConst *args) {
    JSValue global=JS_GetGlobalObject(h->context);
    JSValue fn=JS_GetPropertyStr(h->context,global,name);
    JSValue result=JS_Call(h->context,fn,global,n,args);
    JS_FreeValue(h->context,fn);JS_FreeValue(h->context,global);return result;
}
static int js_error(ScriptHost *h) {
    JSValue error=JS_GetException(h->context);
    const char *text=JS_ToCString(h->context,error);
    lua_pushstring(h->lua,text?text:"Script operation failed");
    JS_FreeCString(h->context,text);JS_FreeValue(h->context,error);return lua_error(h->lua);
}
static int traceback(lua_State *L) {
    const char *message=lua_tostring(L,1);
    if(!message&&lua_istable(L,1)){lua_getfield(L,1,"message");message=lua_tostring(L,-1);}
    luaL_traceback(L,L,message?message:"Lua script failed",1);return 1;
}
static int invocation_error(ScriptHost *h,const char *method) {
    JSValue error=JS_GetException(h->context);const char *text=JS_ToCString(h->context,error);
    lua_State *L=h->lua;lua_newtable(L);lua_pushliteral(L,"InvocationError");lua_setfield(L,-2,"name");
    lua_pushstring(L,text?text:"Script operation failed");lua_setfield(L,-2,"message");
    lua_pushstring(L,method);lua_setfield(L,-2,"methodName");lua_pushboolean(L,1);lua_setfield(L,-2,"__invocation_error__");
    lua_Debug info;if(lua_getstack(L,1,&info)&&lua_getinfo(L,"nl",&info)){
        lua_pushinteger(L,info.currentline);lua_setfield(L,-2,"lineNumber");lua_pushstring(L,info.name?info.name:"");lua_setfield(L,-2,"originFunction");
    }
    JS_FreeCString(h->context,text);JS_FreeValue(h->context,error);return lua_error(L);
}
static JSValue lua_callback(JSContext *ctx,JSValueConst self,int argc,JSValueConst *argv,int magic,JSValue *data) {
    (void)self;(void)magic;
    ScriptHost *h=JS_GetContextOpaque(ctx);lua_State *L=h->lua;
    int top=lua_gettop(L);Callback *callback=JS_GetOpaque(data[0],h->lua_callback_class);
    if(!callback)return JS_ThrowInternalError(ctx,"Lua callback has been released");
    lua_pushcfunction(L,traceback);lua_rawgeti(L,LUA_REGISTRYINDEX,callback->ref);
    for(int i=0;i<argc;i++)if(!to_lua(h,argv[i],0)){lua_settop(L,top);return JS_ThrowInternalError(ctx,"Cannot convert callback argument");}
    if(lua_pcall(L,argc,1,top+1)!=LUA_OK) {
        const char *message=lua_tostring(L,-1);
        JSValue result=JS_ThrowInternalError(ctx,"%s",message?message:"Lua callback failed");
        lua_settop(L,top);return result;
    }
    JSValue result=to_js(h,-1,0);lua_settop(L,top);return result;
}
static JSValue to_js(ScriptHost *h,int index,int depth) {
    lua_State *L=h->lua;JSContext *ctx=h->context;index=lua_absindex(L,index);
    if(depth>64)return JS_ThrowRangeError(ctx,"Lua value nesting exceeds 64");
    switch(lua_type(L,index)) {
    case LUA_TNIL:return JS_UNDEFINED;
    case LUA_TBOOLEAN:return JS_NewBool(ctx,lua_toboolean(L,index));
    case LUA_TNUMBER:return JS_NewFloat64(ctx,lua_tonumber(L,index));
    case LUA_TSTRING:{size_t n;const char *s=lua_tolstring(L,index,&n);return JS_NewStringLen(ctx,s,n);}
    case LUA_TUSERDATA:{
        Object *o=luaL_testudata(L,index,OBJECT);
        return o?JS_DupValue(ctx,o->value):JS_ThrowTypeError(ctx,"Unsupported Lua userdata");
    }
    case LUA_TFUNCTION:{
        lua_pushvalue(L,index);int ref=luaL_ref(L,LUA_REGISTRYINDEX);
        Callback *callback=malloc(sizeof(*callback));if(!callback){luaL_unref(L,LUA_REGISTRYINDEX,ref);return JS_ThrowOutOfMemory(ctx);}
        callback->host=h;callback->ref=ref;
        JSValue data=JS_NewObjectClass(ctx,h->lua_callback_class);
        if(JS_IsException(data)){luaL_unref(L,LUA_REGISTRYINDEX,ref);free(callback);return data;}
        JS_SetOpaque(data,callback);JSValue fn=JS_NewCFunctionData(ctx,lua_callback,0,0,1,&data);
        JS_FreeValue(ctx,data);return fn;
    }
    case LUA_TTABLE:{
        Object *object=lookup(L,index);if(object)return JS_DupValue(ctx,object->value);
        size_t length=lua_rawlen(L,index);
        int array=length>0;JSValue result=array?JS_NewArray(ctx):JS_NewObject(ctx);
        lua_pushnil(L);
        while(lua_next(L,index)) {
            JSValue value=to_js(h,-1,depth+1);
            if(JS_IsException(value)){lua_pop(L,2);JS_FreeValue(ctx,result);return value;}
            if(array&&lua_isinteger(L,-2)&&lua_tointeger(L,-2)>0)JS_SetPropertyUint32(ctx,result,(uint32_t)(lua_tointeger(L,-2)-1),value);
            else if(lua_type(L,-2)==LUA_TSTRING)JS_SetPropertyStr(ctx,result,lua_tostring(L,-2),value);
            else {JS_FreeValue(ctx,value);lua_pop(L,2);JS_FreeValue(ctx,result);return JS_ThrowTypeError(ctx,"Lua tables require string or array keys");}
            lua_pop(L,1);
        }
        return result;
    }
    default:return JS_ThrowTypeError(ctx,"Unsupported Lua value");
    }
}
static int object_gc(lua_State *L) {
    Object *o=luaL_checkudata(L,1,OBJECT);JS_FreeValue(o->host->context,o->value);o->value=JS_UNDEFINED;return 0;
}
static int object_equal(lua_State *L) {
    Object *a=luaL_testudata(L,1,OBJECT),*b=luaL_testudata(L,2,OBJECT);
    lua_pushboolean(L,a&&b&&a->host==b->host&&JS_IsStrictEqual(a->host->context,a->value,b->value));return 1;
}
static int object_call(lua_State *L) {
    Object *o=luaL_checkudata(L,lua_upvalueindex(1),OBJECT);
    ScriptHost *h=o->host;JSContext *ctx=h->context;
    int first=1;Object *self=lookup(L,1);
    if(self&&self->host==h&&JS_IsStrictEqual(ctx,self->value,o->value))first=2;
    JSValue args=JS_NewArray(ctx);int n=lua_gettop(L);
    for(int i=first;i<=n;i++) {
        JSValue v=to_js(h,i,0);
        if(JS_IsException(v)){JS_FreeValue(ctx,args);return js_error(h);}
        JS_SetPropertyUint32(ctx,args,(uint32_t)(i-first),v);
    }
    const char *name=lua_tostring(L,lua_upvalueindex(2));
    JSValue method=JS_NewString(ctx,name);JSValue input[3]={o->value,method,args};
    JSValue result=helper(h,"__luaCall",3,input);
    JS_FreeValue(ctx,method);JS_FreeValue(ctx,args);
    if(JS_IsException(result))return invocation_error(h,name);
    int okay=to_lua(h,result,0);JS_FreeValue(ctx,result);
    if(!okay)return luaL_error(L,"Cannot convert editor result");return 1;
}
static int object_index(lua_State *L) {
    Object *o=luaL_checkudata(L,1,OBJECT);const char *key=luaL_checkstring(L,2);
    JSValue value=JS_GetPropertyStr(o->host->context,o->value,key);
    if(JS_IsException(value))return js_error(o->host);
    if(JS_IsFunction(o->host->context,value)) {
        JS_FreeValue(o->host->context,value);lua_pushvalue(L,1);lua_pushvalue(L,2);lua_pushcclosure(L,object_call,2);return 1;
    }
    int okay=to_lua(o->host,value,0);JS_FreeValue(o->host->context,value);
    if(!okay)return luaL_error(L,"Cannot convert editor property");return 1;
}
static int to_lua(ScriptHost *h,JSValueConst value,int depth) {
    lua_State *L=h->lua;JSContext *ctx=h->context;
    if(depth>64||!lua_checkstack(L,8))return 0;
    if(JS_IsNull(value)||JS_IsUndefined(value)){lua_pushnil(L);return 1;}
    if(JS_IsBool(value)){lua_pushboolean(L,JS_ToBool(ctx,value));return 1;}
    if(JS_IsNumber(value)){double n;if(JS_ToFloat64(ctx,&n,value)<0)return 0;lua_pushnumber(L,n);return 1;}
    if(JS_IsString(value)){size_t n;const char *s=JS_ToCStringLen(ctx,&n,value);if(!s)return 0;lua_pushlstring(L,s,n);JS_FreeCString(ctx,s);return 1;}
    if(!JS_IsObject(value))return 0;
    JSValue input=JS_DupValue(ctx,value);JSValue kind=helper(h,"__bridgeKind",1,&input);JS_FreeValue(ctx,input);
    int is_object=JS_ToBool(ctx,kind);JS_FreeValue(ctx,kind);
    if(is_object) {
        void *identity=JS_VALUE_GET_PTR(value);
        lua_rawgetp(L,LUA_REGISTRYINDEX,&object_cache);lua_rawgetp(L,-1,identity);
        if(lua_istable(L,-1)){lua_remove(L,-2);return 1;}lua_pop(L,2);
        Object *o=lua_newuserdatauv(L,sizeof(*o),0);o->host=h;o->value=JS_DupValue(ctx,value);luaL_setmetatable(L,OBJECT);
        int userdata=lua_gettop(L);lua_newtable(L);int table=lua_gettop(L);
        JSValue input=JS_DupValue(ctx,value);JSValue names=helper(h,"__bridgeMembers",1,&input);JS_FreeValue(ctx,input);
        JSValue len=JS_GetPropertyStr(ctx,names,"length");uint32_t length=0;JS_ToUint32(ctx,&length,len);JS_FreeValue(ctx,len);
        for(uint32_t i=0;i<length;i++) {
            JSValue key=JS_GetPropertyUint32(ctx,names,i);const char *name=JS_ToCString(ctx,key);JS_FreeValue(ctx,key);
            JSValue item=JS_GetPropertyStr(ctx,value,name);
            if(JS_IsFunction(ctx,item)){lua_pushvalue(L,userdata);lua_pushstring(L,name);lua_pushcclosure(L,object_call,2);}
            else if(!to_lua(h,item,depth+1)){JS_FreeCString(ctx,name);JS_FreeValue(ctx,item);JS_FreeValue(ctx,names);return 0;}
            lua_setfield(L,table,name);JS_FreeCString(ctx,name);JS_FreeValue(ctx,item);
        }
        JS_FreeValue(ctx,names);
        lua_pushvalue(L,userdata);lua_setfield(L,table,"__ptr__");
        lua_rawgetp(L,LUA_REGISTRYINDEX,&object_map);lua_pushvalue(L,table);lua_pushvalue(L,userdata);lua_rawset(L,-3);lua_pop(L,1);
        lua_rawgetp(L,LUA_REGISTRYINDEX,&object_cache);lua_pushvalue(L,table);lua_rawsetp(L,-2,identity);lua_pop(L,1);
        lua_remove(L,userdata);return 1;
    }
    lua_newtable(L);int target=lua_gettop(L);
    if(JS_IsArray(value)) {
        JSValue len=JS_GetPropertyStr(ctx,value,"length");uint32_t length;JS_ToUint32(ctx,&length,len);JS_FreeValue(ctx,len);
        for(uint32_t i=0;i<length;i++) {
            JSValue v=JS_GetPropertyUint32(ctx,value,i);int okay=to_lua(h,v,depth+1);JS_FreeValue(ctx,v);
            if(!okay)return 0;lua_rawseti(L,target,(lua_Integer)i+1);
        }
    }else{
        JSPropertyEnum *keys=NULL;uint32_t count=0;
        if(JS_GetOwnPropertyNames(ctx,&keys,&count,value,JS_GPN_STRING_MASK|JS_GPN_ENUM_ONLY)<0)return 0;
        int okay=1;
        for(uint32_t i=0;i<count;i++) {
            const char *key=JS_AtomToCString(ctx,keys[i].atom);
            JSValue v=JS_GetProperty(ctx,value,keys[i].atom);
            if(okay&&key){lua_pushstring(L,key);okay=to_lua(h,v,depth+1);if(okay)lua_rawset(L,target);}
            JS_FreeValue(ctx,v);JS_FreeCString(ctx,key);JS_FreeAtom(ctx,keys[i].atom);
        }
        js_free(ctx,keys);if(!okay)return 0;
    }
    return 1;
}
static JSValue invoke(JSContext *ctx,JSValueConst self,int argc,JSValueConst *argv) {
    (void)self;ScriptHost *h=JS_GetContextOpaque(ctx);lua_State *L=h->lua;
    const char *name=argc?JS_ToCString(ctx,argv[0]):NULL;
    if(!name)return JS_ThrowTypeError(ctx,"Lua function name required");
    int top=lua_gettop(L);lua_pushcfunction(L,traceback);lua_getglobal(L,name);
    int required=!strcmp(name,"main")||!strcmp(name,"getClientInfo")||!strcmp(name,"getSidePanelSectionState");JS_FreeCString(ctx,name);
    if(!lua_isfunction(L,-1)){lua_settop(L,top);return required?JS_ThrowTypeError(ctx,"Required Lua entry point is missing"):JS_UNDEFINED;}
    for(int i=1;i<argc;i++)if(!to_lua(h,argv[i],0)){lua_settop(L,top);return JS_ThrowTypeError(ctx,"Cannot convert Lua argument");}
    if(lua_pcall(L,argc-1,1,top+1)!=LUA_OK) {
        const char *error=lua_tostring(L,-1);JSValue v=JS_ThrowInternalError(ctx,"%s",error?error:"Lua script failed");lua_settop(L,top);return v;
    }
    JSValue result=to_js(h,-1,0);lua_settop(L,top);return result;
}
static void interrupt(lua_State *L,lua_Debug *debug) {
    (void)debug;ScriptHost *h=*(ScriptHost **)lua_getextraspace(L);
    if(h->cancel(h->opaque))luaL_error(L,"Script stopped");
}
int script_lua_load(ScriptHost *h,const char *source,size_t length,const char *name) {
    if(!h->lua) {
        JS_NewClassID(h->runtime,&h->lua_callback_class);
        JSClassDef callback_class={.class_name="LuaCallback",.finalizer=callback_free};
        if(JS_NewClass(h->runtime,h->lua_callback_class,&callback_class)<0)return script_result(h,JS_ThrowOutOfMemory(h->context));
        h->lua=lua_newstate(lua_allocate,h);if(!h->lua)return script_result(h,JS_ThrowOutOfMemory(h->context));
        lua_State *L=h->lua;*(ScriptHost **)lua_getextraspace(L)=h;
        luaL_openlibs(L);lua_sethook(L,interrupt,LUA_MASKCOUNT,10000);
        luaL_newmetatable(L,OBJECT);
        lua_pushcfunction(L,object_index);lua_setfield(L,-2,"__index");
        lua_pushcfunction(L,object_gc);lua_setfield(L,-2,"__gc");
        lua_pushcfunction(L,object_equal);lua_setfield(L,-2,"__eq");lua_pop(L,1);
        lua_newtable(L);lua_newtable(L);lua_pushliteral(L,"k");lua_setfield(L,-2,"__mode");lua_setmetatable(L,-2);lua_rawsetp(L,LUA_REGISTRYINDEX,&object_map);
        lua_newtable(L);lua_newtable(L);lua_pushliteral(L,"v");lua_setfield(L,-2,"__mode");lua_setmetatable(L,-2);lua_rawsetp(L,LUA_REGISTRYINDEX,&object_cache);
        JSValue global=JS_GetGlobalObject(h->context);
        JSValue sv=JS_GetPropertyStr(h->context,global,"SV");to_lua(h,sv,0);lua_setglobal(L,"SV");JS_FreeValue(h->context,sv);
        JS_SetPropertyStr(h->context,global,"__luaInvoke",JS_NewCFunction(h->context,invoke,"__luaInvoke",1));JS_FreeValue(h->context,global);
    }
    lua_State *L=h->lua;
    const char *slash=strrchr(name,'/'),*backslash=strrchr(name,'\\');if(!slash||(backslash&&backslash>slash))slash=backslash;
    if(slash){
        lua_getglobal(L,"package");lua_getfield(L,-1,"path");const char *old=lua_tostring(L,-1);
        lua_pushlstring(L,name,(size_t)(slash-name+1));lua_pushliteral(L,"?.lua;");lua_pushlstring(L,name,(size_t)(slash-name+1));lua_pushliteral(L,"?/init.lua;");lua_pushstring(L,old?old:"");lua_concat(L,5);lua_setfield(L,-3,"path");lua_pop(L,2);
    }
    int base=lua_gettop(L);lua_pushcfunction(L,traceback);
    if(luaL_loadbufferx(L,source,length,name,"t")!=LUA_OK||lua_pcall(L,0,0,base+1)!=LUA_OK) {
        const char *error=lua_tostring(L,-1);JSValue v=JS_ThrowInternalError(h->context,"%s",error?error:"Lua script failed");lua_settop(L,base);return script_result(h,v);
    }
    lua_settop(L,base);
    const char *wrappers="function getClientInfo(){return __luaInvoke('getClientInfo');}"
        "function getTranslations(language){return __luaInvoke('getTranslations',language);}"
        "function getSidePanelSectionState(){return __luaInvoke('getSidePanelSectionState');}"
        "function main(){return __luaInvoke('main');}";
    return script_result(h,JS_Eval(h->context,wrappers,strlen(wrappers),"Lua entry points",JS_EVAL_TYPE_GLOBAL));
}
void script_lua_destroy(ScriptHost *h){if(h->lua){lua_State *L=h->lua;h->lua=NULL;lua_close(L);}}
