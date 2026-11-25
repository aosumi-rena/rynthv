#include "host.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* The UI validates a single conversion before calling this formatter. */
int rynthv_script_format_number(const char *format,double value,int integer,char *output,size_t size) {
    return integer?snprintf(output,size,format,(int)value):snprintf(output,size,format,value);
}

static int interrupted(JSContext *context, void *opaque) {
    ScriptHost *host=opaque;
    (void)context;
    return host->cancel(host->opaque);
}
static JSValue request(JSContext *context, JSValueConst self, int argc, JSValueConst *argv) {
    ScriptHost *host=JS_GetContextOpaque(context);
    (void)self;
    if(argc!=1) return JS_ThrowTypeError(context,"Invalid editor request");
    const char *input=JS_ToCString(context,argv[0]);
    if(!input) return JS_EXCEPTION;
    const char *output=host->request(host->opaque,input);
    JS_FreeCString(context,input);
    if(!output) return JS_ThrowInternalError(context,"Script cancelled");
    return JS_NewString(context,output);
}
ScriptHost *rynthv_script_create(request_fn request_callback,cancel_fn cancel_callback,void *opaque) {
    ScriptHost *host=calloc(1,sizeof(*host));
    if(!host)return NULL;
    host->request=request_callback;host->cancel=cancel_callback;host->opaque=opaque;
    host->runtime=JS_NewRuntime();
    if(!host->runtime){free(host);return NULL;}
    JS_SetMemoryLimit(host->runtime,256*1024*1024);
    JS_SetMaxStackSize(host->runtime,512*1024);
    JS_SetInterruptHandler(host->runtime,interrupted,host);
    host->context=JS_NewContext(host->runtime);
    if(!host->context){JS_FreeRuntime(host->runtime);free(host);return NULL;}
    JS_SetContextOpaque(host->context,host);
    JSValue global=JS_GetGlobalObject(host->context);
    JS_SetPropertyStr(host->context,global,"__host",JS_NewCFunction(host->context,request,"__host",1));
    JS_FreeValue(host->context,global);
    return host;
}
int script_result(ScriptHost *host,JSValue value) {
    free(host->result);host->result=NULL;
    int failed=JS_IsException(value);
    if(failed) {
        value=JS_GetException(host->context);
        JSValue stack=JS_GetPropertyStr(host->context,value,"stack");
        JSValue message=JS_ToString(host->context,value);
        const char *a=JS_ToCString(host->context,message),*b=JS_ToCString(host->context,stack);
        if(a){size_t n=strlen(a),m=b?strlen(b):0;host->result=malloc(n+m+2);
            if(host->result){memcpy(host->result,a,n);host->result[n]='\n';if(b)memcpy(host->result+n+1,b,m);host->result[n+m+1]=0;}}
        JS_FreeCString(host->context,a);JS_FreeCString(host->context,b);
        JS_FreeValue(host->context,message);JS_FreeValue(host->context,stack);
    } else {
        const char *text=JS_ToCString(host->context,value);
        if(text){size_t n=strlen(text);host->result=malloc(n+1);if(host->result)memcpy(host->result,text,n+1);JS_FreeCString(host->context,text);}
    }
    JS_FreeValue(host->context,value);
    return failed;
}
int rynthv_script_eval(ScriptHost *host,const char *source,size_t length,const char *name) {
    int failed=script_result(host,JS_Eval(host->context,source,length,name,JS_EVAL_TYPE_GLOBAL));
    if(failed)return failed;
    JSContext *job=NULL;int status;
    while((status=JS_ExecutePendingJob(host->runtime,&job))>0){}
    if(status<0){JSValue error=JS_GetException(job?job:host->context);JS_Throw(host->context,error);return script_result(host,JS_EXCEPTION);}
    return 0;
}
int rynthv_script_load_lua(ScriptHost *host,const char *source,size_t length,const char *name) {
    return script_lua_load(host,source,length,name);
}
const char *rynthv_script_result(ScriptHost *host){return host->result?host->result:"Script allocation failed";}
void rynthv_script_destroy(ScriptHost *host) {
    if(!host)return;
    script_lua_destroy(host);
    JS_FreeContext(host->context);JS_FreeRuntime(host->runtime);free(host->result);free(host);
}
