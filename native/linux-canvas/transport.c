#define _GNU_SOURCE
#include "transport.h"
#include <sys/file.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>

struct aria_canvas { int fd; uint8_t *map; size_t bytes; char path[192]; };
static const char magic[8] = "ARIACV01";
static void put32(uint8_t *p, uint32_t v) { memcpy(p, &v, 4); }
static uint32_t get32(const uint8_t *p) { uint32_t v; memcpy(&v, p, 4); return v; }
static uint64_t get64(const uint8_t *p) { uint64_t v; memcpy(&v, p, 8); return v; }
static int valid(uint32_t w, uint32_t h, uint32_t f) { return w && h && w <= ARIA_MAX_EDGE && h <= ARIA_MAX_EDGE && (f == 1 || f == 2); }
struct aria_canvas *aria_canvas_create(const char *name, uint32_t w, uint32_t h, uint32_t format) {
    if (!valid(w, h, format)) return NULL;
    struct aria_canvas *c = calloc(1, sizeof(*c));
    if (!c) return NULL;
    snprintf(c->path, sizeof(c->path), "/dev/shm/aria-canvas-%u-%u-XXXXXX", (unsigned)getuid(), (unsigned)getpid());
    c->fd = mkostemp(c->path, O_CLOEXEC);
    c->bytes = ARIA_HEADER_SIZE + (size_t)w * h * 4;
    if (c->fd < 0) { free(c); return NULL; }
    /* Reserve tmpfs space before mmap writes: a full /dev/shm must report an
     * error, never SIGBUS while publishing a large canvas. */
    if (posix_fallocate(c->fd, 0, (off_t)c->bytes) != 0) goto fail;
    c->map = mmap(NULL, c->bytes, PROT_READ | PROT_WRITE, MAP_SHARED, c->fd, 0);
    if (c->map == MAP_FAILED) { c->map = NULL; goto fail; }
    if (flock(c->fd, LOCK_EX | LOCK_NB)) goto fail;
    memset(c->map, 0, ARIA_HEADER_SIZE);
    memcpy(c->map, magic, 8);
    put32(c->map + 8, w); put32(c->map + 12, h); put32(c->map + 16, format);
    snprintf((char *)c->map + 32, 128, "%s", name);
    flock(c->fd, LOCK_UN);
    return c;
fail:
    aria_canvas_destroy(c); return NULL;
}
const char *aria_canvas_path(struct aria_canvas *c) { return c->path; }
int aria_canvas_write(struct aria_canvas *c, const uint8_t *pixels, size_t stride) {
    if (flock(c->fd, LOCK_EX | LOCK_NB)) return errno == EWOULDBLOCK ? 0 : -1;
    uint32_t w = get32(c->map + 8), h = get32(c->map + 12);
    if (stride < (size_t)w * 4) { flock(c->fd, LOCK_UN); return -1; }
    for (uint32_t y = 0; y < h; y++) memcpy(c->map + ARIA_HEADER_SIZE + (size_t)y*w*4, pixels + y*stride, (size_t)w*4);
    uint64_t serial = get64(c->map + 24) + 1;
    memcpy(c->map + 24, &serial, 8);
    flock(c->fd, LOCK_UN);
    return 1;
}
void aria_canvas_destroy(struct aria_canvas *c) {
    if (!c) return;
    unlink(c->path);
    if (c->map) munmap(c->map, c->bytes);
    close(c->fd); free(c);
}
int aria_canvas_read(const char *path, uint8_t **pixels, uint32_t *w, uint32_t *h, uint32_t *format, uint64_t *serial) {
    int fd = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
    if (fd < 0) return -1;
    int result = -1;
    struct stat st;
    uint8_t header[ARIA_HEADER_SIZE];
    if (fstat(fd, &st) || !S_ISREG(st.st_mode) || st.st_uid != getuid() || (st.st_mode & 077) || st.st_size < ARIA_HEADER_SIZE) goto done;
    if (flock(fd, LOCK_EX | LOCK_NB)) { result = errno == EWOULDBLOCK ? 0 : -1; goto done; }
    if (pread(fd, header, sizeof(header), 0) != (ssize_t)sizeof(header) || memcmp(header, magic, 8)) goto done;
    uint32_t nw = get32(header + 8), nh = get32(header + 12), nf = get32(header + 16);
    if (!valid(nw, nh, nf) || st.st_size != (off_t)(ARIA_HEADER_SIZE + (size_t)nw*nh*4)) goto done;
    uint64_t ns = get64(header + 24);
    if (!ns || (ns == *serial && nw == *w && nh == *h && nf == *format)) { result = 0; goto done; }
    size_t bytes = (size_t)nw*nh*4;
    if (!*pixels || nw != *w || nh != *h) {
        uint8_t *next = realloc(*pixels, bytes);
        if (!next) goto done;
        *pixels = next;
    }
    if (pread(fd, *pixels, bytes, ARIA_HEADER_SIZE) != (ssize_t)bytes) goto done;
    *w = nw; *h = nh; *format = nf; *serial = ns;
    result = 1;
done:
    close(fd); /* also releases flock, including error paths */
    if (result < 0) {
        /* A failed read after realloc must not leave dimensions describing an
         * older, larger allocation. Clear both sides of the capacity contract. */
        free(*pixels); *pixels = NULL; *w = *h = *format = 0; *serial = 0;
    }
    return result;
}
