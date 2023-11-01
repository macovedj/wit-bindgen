// Generated by `wit-bindgen` 0.13.0. DO NOT EDIT!
#ifndef __BINDINGS_FOO_H
#define __BINDINGS_FOO_H
#ifdef __cplusplus
extern "C" {
#endif

#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>

typedef struct {
  uint8_t*ptr;
  size_t len;
} foo_string_t;

// Exported Functions from `foo`
void foo_concat(foo_string_t *left, foo_string_t *right, foo_string_t *ret);

// Helper Functions

// Transfers ownership of `s` into the string `ret`
void foo_string_set(foo_string_t *ret, char*s);

// Creates a copy of the input nul-terminate string `s` and
// stores it into the component model string `ret`.
void foo_string_dup(foo_string_t *ret, const char*s);

// Deallocates the string pointed to by `ret`, deallocating
// the memory behind the string.
void foo_string_free(foo_string_t *ret);

#ifdef __cplusplus
}
#endif
#endif
