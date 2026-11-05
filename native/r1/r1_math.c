/* SPDX-License-Identifier: GPL-3.0-or-later */
#include <stdint.h>
#include <xmmintrin.h>
#if defined(_MSC_VER) && !defined(__clang__)
#include <intrin.h>
#endif
/* Preserve the original instruction's destination when its zero-input result
   is CPU-specific. This executes only our own arithmetic helper. */
__declspec(dllexport) uint32_t r1_math_bsr(uint32_t value) {
#if defined(_MSC_VER) && !defined(__clang__)
  unsigned long index=value;
  _BitScanReverse(&index,value);
  value=(uint32_t)index;
#else
  __asm__ volatile("bsrl %0, %0" : "+r"(value) : : "cc");
#endif
  return value;
}
__declspec(dllexport) uint32_t r1_math_mxcsr(void) { return _mm_getcsr(); }
