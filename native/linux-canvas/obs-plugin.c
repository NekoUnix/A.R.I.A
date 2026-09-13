/* SPDX-License-Identifier: GPL-2.0-or-later
 * Original ARIA OBS plugin; separately licensed from the MIT application. */
#define _GNU_SOURCE
#include <obs-module.h>
#include <graphics/graphics.h>
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/file.h>
#include <unistd.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "transport.h"
OBS_DECLARE_MODULE()
MODULE_EXPORT const char *obs_module_description(void) { return "ARIA full-resolution Linux canvas input (Alpha)"; }
struct input {
    pthread_mutex_t lock;
    char *name;
    char path[256];
    uint8_t *pixels;
    uint32_t w, h, format;
    uint64_t serial;
    gs_texture_t *texture;
    bool dirty;
    float retry;
};
static const char *source_name(void *unused) { (void)unused; return "ARIA Canvas (Alpha)"; }
static int describe(const char *path, char name[128]) {
    int fd = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK);
    if (fd < 0) return 0;
    struct stat st;
    uint8_t header[ARIA_HEADER_SIZE];
    int ok = !fstat(fd, &st) && S_ISREG(st.st_mode) && st.st_uid == getuid() && !(st.st_mode & 077)
        && !flock(fd, LOCK_EX | LOCK_NB) && pread(fd, header, sizeof(header), 0) == sizeof(header) && !memcmp(header, "ARIACV01", 8);
    close(fd);
    if (ok) { memcpy(name, header + 32, 127); name[127] = 0; }
    return ok;
}
static void discover(obs_property_t *list, const char *wanted, char *found) {
    DIR *dir = opendir("/dev/shm");
    if (!dir) return;
    struct dirent *entry;
    char prefix[64];
    snprintf(prefix, sizeof(prefix), "aria-canvas-%u-", (unsigned)getuid());
    while ((entry = readdir(dir))) {
        if (strncmp(entry->d_name, prefix, strlen(prefix))) continue;
        char path[512], name[128];
        snprintf(path, sizeof(path), "/dev/shm/%s", entry->d_name);
        if (!describe(path, name)) continue;
        if (list) obs_property_list_add_string(list, name, name);
        if (wanted && !strcmp(wanted, name) && strlen(path) < 256) { strcpy(found, path); break; }
    }
    closedir(dir);
}
static obs_properties_t *properties(void *data) {
    (void)data;
    obs_properties_t *props = obs_properties_create();
    obs_property_t *list = obs_properties_add_list(props, "sender", "ARIA sender (open its output first)", OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_STRING);
    obs_property_list_add_string(list, "Select an open ARIA output", "");
    discover(list, NULL, NULL);
    return props;
}
static void update(void *data, obs_data_t *settings) {
    struct input *s = data;
    pthread_mutex_lock(&s->lock);
    free(s->name); s->name = strdup(obs_data_get_string(settings, "sender"));
    s->path[0] = 0; s->serial = 0; s->w = s->h = 0; s->retry = 0;
    pthread_mutex_unlock(&s->lock);
}
static void *create(obs_data_t *settings, obs_source_t *source) {
    (void)source;
    struct input *s = calloc(1, sizeof(*s));
    if (!s) return NULL;
    pthread_mutex_init(&s->lock, NULL); update(s, settings); return s;
}
static void destroy(void *data) {
    struct input *s = data;
    obs_enter_graphics(); gs_texture_destroy(s->texture); obs_leave_graphics();
    free(s->pixels); free(s->name); pthread_mutex_destroy(&s->lock); free(s);
}
static void tick(void *data, float seconds) {
    struct input *s = data;
    if (pthread_mutex_trylock(&s->lock)) return;
    s->retry -= seconds;
    if (!s->path[0] && s->retry <= 0) { discover(NULL, s->name, s->path); s->retry = 1.0f; }
    if (s->path[0]) {
        int result = aria_canvas_read(s->path, &s->pixels, &s->w, &s->h, &s->format, &s->serial);
        if (result > 0) s->dirty = true;
        else if (result < 0) { s->path[0] = 0; s->serial = 0; s->w = s->h = 0; }
    }
    pthread_mutex_unlock(&s->lock);
}
static void render(void *data, gs_effect_t *effect) {
    (void)effect;
    struct input *s = data;
    if (pthread_mutex_trylock(&s->lock)) return;
    if (s->w && s->h && s->serial) {
        enum gs_color_format fmt = s->format == 1 ? GS_BGRA : GS_RGBA;
        if (s->texture && (gs_texture_get_width(s->texture) != s->w || gs_texture_get_height(s->texture) != s->h || gs_texture_get_color_format(s->texture) != fmt)) {
            gs_texture_destroy(s->texture); s->texture = NULL;
        }
        if (!s->texture) s->texture = gs_texture_create(s->w, s->h, fmt, 1, NULL, GS_DYNAMIC);
        if (s->texture) {
            if (s->dirty) { gs_texture_set_image(s->texture, s->pixels, s->w * 4, false); s->dirty = false; }
            gs_blend_state_push(); gs_blend_function(GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);
            gs_effect_t *draw = obs_get_base_effect(OBS_EFFECT_DEFAULT);
            gs_effect_set_texture(gs_effect_get_param_by_name(draw, "image"), s->texture);
            while (gs_effect_loop(draw, "Draw")) gs_draw_sprite(s->texture, 0, s->w, s->h);
            gs_blend_state_pop();
        }
    }
    pthread_mutex_unlock(&s->lock);
}
static uint32_t width(void *data) { struct input *s = data; pthread_mutex_lock(&s->lock); uint32_t v = s->w; pthread_mutex_unlock(&s->lock); return v; }
static uint32_t height(void *data) { struct input *s = data; pthread_mutex_lock(&s->lock); uint32_t v = s->h; pthread_mutex_unlock(&s->lock); return v; }
bool obs_module_load(void) {
    struct obs_source_info info = { .id = "aria_canvas_v1", .type = OBS_SOURCE_TYPE_INPUT, .output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
        .get_name = source_name, .create = create, .destroy = destroy, .update = update, .get_properties = properties,
        .video_tick = tick, .video_render = render, .get_width = width, .get_height = height };
    obs_register_source(&info); return true;
}
