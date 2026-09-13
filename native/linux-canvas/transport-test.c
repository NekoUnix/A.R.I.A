#define _GNU_SOURCE
#include "transport.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/file.h>
#include <sys/wait.h>
#include <fcntl.h>
#include <stdio.h>
int main(void) {
    assert(!aria_canvas_create("invalid", 0, 10, 1));
    assert(!aria_canvas_create("huge", 4097, 10, 1));
    struct aria_canvas *c = aria_canvas_create("test", 65, 2, 1);
    assert(c);
    char *path = strdup(aria_canvas_path(c));
    uint8_t padded[1024]; memset(padded, 128, sizeof(padded));
    padded[0] = 1; padded[512] = 2;
    assert(aria_canvas_write(c, padded, 512) == 1);
    /* Separate process verifies publication, row padding and contention. */
    pid_t pid = fork(); assert(pid >= 0);
    if (!pid) {
        uint8_t *pixels = NULL; uint32_t w=0,h=0,f=0; uint64_t serial=0;
        assert(aria_canvas_read(path, &pixels, &w,&h,&f,&serial) == 1);
        assert(w == 65 && h == 2 && f == 1 && serial == 1);
        assert(pixels[0] == 1 && pixels[260] == 2 && pixels[3] == 128);
        assert(aria_canvas_read(path, &pixels, &w,&h,&f,&serial) == 0);
        int fd = open(path, O_RDONLY); assert(fd >= 0); assert(!flock(fd, LOCK_EX));
        assert(aria_canvas_read(path, &pixels, &w,&h,&f,&serial) == 0);
        close(fd); free(pixels); _exit(0);
    }
    int status; assert(waitpid(pid,&status,0) == pid); assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    aria_canvas_destroy(c); assert(access(path, F_OK)); free(path);
    puts("Canvas protocol: process transfer, stride, alpha bytes, contention, bounds and cleanup passed.");
}
