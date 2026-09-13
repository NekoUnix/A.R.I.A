/* Original ARIA MIT-licensed local canvas protocol, version 1. */
#pragma once
#include <stdint.h>
#include <stddef.h>
#define ARIA_HEADER_SIZE 256u
#define ARIA_MAX_EDGE 4096u
/* Little-endian header: magic ARIACV01[8], width u32, height u32,
 * format u32 (1=BGRA,2=RGBA premultiplied), reserved u32, serial u64,
 * name[128], padding[96]. Rows are tightly packed, top to bottom.
 * Every header and pixel access must hold flock(LOCK_EX). */
struct aria_canvas;
struct aria_canvas *aria_canvas_create(const char *name, uint32_t w, uint32_t h, uint32_t format);
const char *aria_canvas_path(struct aria_canvas *canvas);
int aria_canvas_write(struct aria_canvas *canvas, const uint8_t *pixels, size_t stride);
void aria_canvas_destroy(struct aria_canvas *canvas);
/* Reader returns: 1=new frame, 0=busy/no change, -1=gone/invalid.
 * Output storage is resized only when dimensions change. */
int aria_canvas_read(const char *path, uint8_t **pixels, uint32_t *w, uint32_t *h, uint32_t *format, uint64_t *serial);
