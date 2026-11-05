/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef R1_LLSM2_BRIDGE_H
#define R1_LLSM2_BRIDGE_H
#include <stdint.h>
#ifdef _WIN32
#define R1_L2_API __declspec(dllexport)
#else
#define R1_L2_API
#endif
typedef struct {
  int32_t abi, frames, harmonics, envelope_harmonics, noise_bins, bands;
  float hop_seconds, nyquist, lip_radius;
} r1_llsm2_config;
/* Packed floats: bands-1 edges, then each frame: f0, rd, nhar, has_noise,
   1025 VT bins, harmonics source phases, bands counts, noise_bins PSD,
   bands * (DC, envelope_harmonics amplitudes, envelope_harmonics phases).
   This ABI represents already prepared layer1, not compressed S5D bytes. */
R1_L2_API int32_t r1_llsm2_samples(const r1_llsm2_config* config);
R1_L2_API int32_t r1_llsm2_noise_axis(int32_t count, float nyquist, float warp, float* output);
R1_L2_API int32_t r1_llsm2_source_phases(float rd, float original_f0, float target_f0, int32_t count, float* output);
R1_L2_API int32_t r1_llsm2_render(const r1_llsm2_config* config, const float* data,
  int32_t count, float* output, int32_t capacity);
typedef int32_t (*r1_llsm2_callback)(void*, int32_t, int32_t, const float*, const float*);
R1_L2_API int32_t r1_llsm2_render_stream(const r1_llsm2_config*, const float*, int32_t,
  float*, int32_t, r1_llsm2_callback, void*);
#endif
