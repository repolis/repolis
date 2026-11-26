package lang

/*
#include <stdlib.h>

void ts_set_allocator(void *(*)(size_t), void *(*)(size_t, size_t), void *(*)(void *, size_t), void (*)(void *));

static void use_libc_allocator(void) { ts_set_allocator(malloc, calloc, realloc, free); }
*/
import "C"

func init() { C.use_libc_allocator() }
