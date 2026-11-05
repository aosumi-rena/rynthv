/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "r1_llsm2.h"
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include "vendor/libllsm2/llsm.h"
#include "vendor/libllsm2/dsputils.h"
#include "vendor/external/ciglet/ciglet.h"

int32_t r1_llsm2_source_phases(float rd, float original_f0, float target_f0, int32_t count, float* output) {
  if(!output || !isfinite(rd) || rd<=0 || rd>10 || !isfinite(original_f0) || original_f0<20 ||
     !isfinite(target_f0) || target_f0<30 || count<1 || count>1600) return -1;
  float* frequency=malloc(count*sizeof(float));if(!frequency)return -2;
  for(int i=0;i<count;i++)frequency[i]=(float)(((double)i+1.0)*(double)target_f0);
  lfmodel model=lfmodel_from_rd(rd,1.0f/original_f0,1.0f);
  float* amplitude=lfmodel_spectrum(model,frequency,count,output);
  free(frequency);if(!amplitude)return -2;free(amplitude);
  for(int i=0;i<count;i++)if(!isfinite(output[i]))return -3;
  return 0;
}

int32_t r1_llsm2_noise_axis(int32_t count, float nyquist, float warp, float* output) {
  if(!output || count<4 || count>512 || !isfinite(nyquist) || nyquist<8000 || nyquist>48000 ||
     !isfinite(warp) || warp<=0) return -1;
  float* axis=llsm_warp_frequency(0,nyquist,count,warp);
  if(!axis)return -2;
  memcpy(output,axis,count*sizeof(float));free(axis);return 0;
}

int32_t r1_llsm2_samples(const r1_llsm2_config* c) {
  if(!c || c->abi!=1 || c->frames<2 || c->frames>8192 ||
     c->harmonics<1 || c->harmonics>512 || c->envelope_harmonics<0 || c->envelope_harmonics>32 ||
     c->noise_bins<4 || c->noise_bins>512 || c->bands<2 || c->bands>8 ||
     !isfinite(c->hop_seconds) || c->hop_seconds<0.001f || c->hop_seconds>0.02f ||
     !isfinite(c->nyquist) || c->nyquist<8000 || c->nyquist>48000 ||
     !isfinite(c->lip_radius) || c->lip_radius<=0 || c->lip_radius>10) return -1;
  /* Same separately rounded float expression as this source's synthesis. */
  float time=(c->frames+1)*c->hop_seconds;
  float length=time*(c->nyquist*2.0f);
  int32_t n=(int32_t)round((double)length);
  return n>0 && n<=4194304 ? n : -1;
}
static int validate(const r1_llsm2_config* c,const float* d,int32_t count) {
  int stride=4+1025+c->harmonics+c->bands+c->noise_bins+c->bands*(1+2*c->envelope_harmonics);
  if(!d || count!=c->bands-1+c->frames*stride) return -2;
  for(int i=0;i<count;i++) if(!isfinite(d[i])) return -3;
  float last=0;
  for(int b=0;b<c->bands-1;b++) { if(d[b]<=last || d[b]>=c->nyquist)return -3;last=d[b]; }
  const float* p=d+c->bands-1;
  for(int i=0;i<c->frames;i++,p+=stride) {
    float f0=p[0],rd=p[1],nh=p[2],noise=p[3];
    if(f0<0 || (f0>0 && f0<20) || f0>c->nyquist || (f0>0 && (rd<=0 || rd>10)) ||
       nh<0 || nh>c->harmonics || nh!=floorf(nh) || (f0>0 && nh<1) ||
       (noise!=0 && noise!=1))return -3;
    /* Phase storage can exceed the new pitch's physical harmonics. The source
       layer1-to0 conversion clips nhar to both MAXNHAR and floor(Fnyq/F0). */
    for(int k=0;k<1025;k++) if(fabsf(p[4+k])>200)return -3;
    const float* counts=p+4+1025+c->harmonics;
    for(int b=0;b<c->bands;b++)if(counts[b]<0 || counts[b]>c->envelope_harmonics || counts[b]!=floorf(counts[b]))return -3;
    const float* psd=counts+c->bands;
    for(int k=0;k<c->noise_bins;k++)if(fabsf(psd[k])>200)return -3;
    const float* e=psd+c->noise_bins;
    for(int b=0;b<c->bands;b++,e+=1+2*c->envelope_harmonics) {
      if(e[0]<0)return -3;
      for(int k=0;k<c->envelope_harmonics;k++)if(e[1+k]<0)return -3;
    }
  }
  return 0;
}
static void integer(llsm_container* c,int slot,int value) {
  llsm_container_attach(c,slot,llsm_create_int(value),llsm_delete_int,llsm_copy_int);
}
static void scalar(llsm_container* c,int slot,float value) {
  llsm_container_attach(c,slot,llsm_create_fp(value),llsm_delete_fp,llsm_copy_fp);
}
static void vector(llsm_container* c,int slot,const float* data,int count) {
  float* a=llsm_create_fparray(count);memcpy(a,data,count*sizeof(float));
  llsm_container_attach(c,slot,a,llsm_delete_fparray,llsm_copy_fparray);
}
int32_t r1_llsm2_render_stream(const r1_llsm2_config* c,const float* d,int32_t count,float* output,int32_t capacity,
    r1_llsm2_callback callback, void* user) {
  int n=r1_llsm2_samples(c);if(n<0)return -1;
  if(!output || capacity!=n*3)return -2;
  int code=validate(c,d,count);if(code)return code;
  llsm_container* conf=llsm_create_container(12);
  integer(conf,LLSM_CONF_NFRM,c->frames);scalar(conf,LLSM_CONF_THOP,c->hop_seconds);
  integer(conf,LLSM_CONF_MAXNHAR,c->harmonics);integer(conf,LLSM_CONF_MAXNHAR_E,c->envelope_harmonics);
  integer(conf,LLSM_CONF_NPSD,c->noise_bins);scalar(conf,LLSM_CONF_FNYQ,c->nyquist);
  integer(conf,LLSM_CONF_NCHANNEL,c->bands);integer(conf,LLSM_CONF_NSPEC,1025);
  scalar(conf,LLSM_CONF_LIPRADIUS,c->lip_radius);vector(conf,LLSM_CONF_CHANFREQ,d,c->bands-1);
  /* Chunk copies conf. Start without default frames to avoid replacing/leaking them. */
  llsm_chunk* chunk=llsm_create_chunk(conf,0);llsm_delete_container(conf);
  const float* p=d+c->bands-1;
  for(int i=0;i<c->frames;i++) {
    float f0=*p++,rd=*p++;int nh=(int)*p++,has_noise=(int)*p++;
    llsm_container* frame=llsm_create_frame(nh,c->bands,c->envelope_harmonics,c->noise_bins);
    chunk->frames[i]=frame;scalar(frame,LLSM_FRAME_F0,f0);scalar(frame,LLSM_FRAME_RD,rd);
    if(f0>0 && nh>0) {
      vector(frame,LLSM_FRAME_VTMAGN,p,1025);
      vector(frame,LLSM_FRAME_VSPHSE,p+1025,nh);
    }
    p+=1025+c->harmonics;
    const float* counts=p;p+=c->bands;
    llsm_nmframe* noise=llsm_container_get(frame,LLSM_FRAME_NM);
    if(has_noise)memcpy(noise->psd,p,c->noise_bins*sizeof(float));p+=c->noise_bins;
    for(int b=0;b<c->bands;b++) {
      if(has_noise) {
        noise->edc[b]=p[0];noise->eenv[b]->nhar=(int)counts[b];
        memcpy(noise->eenv[b]->ampl,p+1,c->envelope_harmonics*sizeof(float));
        memcpy(noise->eenv[b]->phse,p+1+c->envelope_harmonics,c->envelope_harmonics*sizeof(float));
      }
      p+=1+2*c->envelope_harmonics;
    }
  }
  llsm_chunk_tolayer0(chunk);llsm_chunk_phasepropagate(chunk,1);
  llsm_soptions* options=llsm_create_soptions(c->nyquist*2.0f);options->use_l1=0;
  int cancelled=0;
  llsm_output* out=llsm_synthesize_stream(options,chunk,callback,user,&cancelled);llsm_delete_soptions(options);
  if(cancelled)code=-5;
  else if(!out || out->ny!=n)code=-4;
  else {
    for(int i=0;i<n;i++)if(!isfinite(out->y[i])||!isfinite(out->y_sin[i])||!isfinite(out->y_noise[i])) {code=-4;break;}
    if(!code) {memcpy(output,out->y,n*sizeof(float));memcpy(output+n,out->y_sin,n*sizeof(float));memcpy(output+2*n,out->y_noise,n*sizeof(float));}
  }
  llsm_delete_output(out);llsm_delete_chunk(chunk);return code;
}

int32_t r1_llsm2_render(const r1_llsm2_config* c,const float* d,int32_t count,float* output,int32_t capacity) {
  return r1_llsm2_render_stream(c,d,count,output,capacity,NULL,NULL);
}
