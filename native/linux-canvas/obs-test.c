#define _GNU_SOURCE
#include <obs.h>
#include <obs-module.h>
#include <graphics/graphics.h>
#include <graphics/vec4.h>
#include <assert.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "transport.h"
int main(int argc, char **argv) {
    assert(argc == 2);
    assert(obs_startup("en-US", NULL, NULL));
    struct obs_video_info video = { .graphics_module = "libobs-opengl", .fps_num=30, .fps_den=1,
        .base_width=128, .base_height=128, .output_width=128, .output_height=128,
        .output_format=VIDEO_FORMAT_RGBA, .colorspace=VIDEO_CS_709, .range=VIDEO_RANGE_FULL,
        .scale_type=OBS_SCALE_BILINEAR };
    assert(obs_reset_video(&video) == OBS_VIDEO_SUCCESS);
    obs_module_t *module = NULL;
    assert(obs_open_module(&module, argv[1], ".") == MODULE_SUCCESS);
    assert(obs_init_module(module));
    obs_data_t *settings = obs_data_create();
    obs_data_set_string(settings, "sender", "ARIA OBS CI");
    obs_source_t *source = obs_source_create("aria_canvas_v1", "Test native receiver", settings, NULL);
    assert(source); obs_data_release(settings);
    obs_set_output_source(0, source);
    for (int shape = 0; shape < 2; shape++) {
        uint32_t w = shape ? 128 : 65, h = shape ? 65 : 128;
        struct aria_canvas *canvas = aria_canvas_create("ARIA OBS CI", w, h, 1);
        assert(canvas);
        uint8_t *pixels = malloc((size_t)w*h*4);
        for (size_t i=0;i<(size_t)w*h;i++) { pixels[i*4]=16; pixels[i*4+1]=32; pixels[i*4+2]=64; pixels[i*4+3]=128; }
        assert(aria_canvas_write(canvas, pixels, w*4) == 1); free(pixels);
        for (int wait=0;wait<100 && obs_source_get_width(source)!=w;wait++) usleep(30000);
        assert(obs_source_get_width(source)==w && obs_source_get_height(source)==h);
        obs_enter_graphics();
        gs_texrender_t *target = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
        assert(gs_texrender_begin(target, w, h));
        struct vec4 clear; vec4_zero(&clear); gs_clear(GS_CLEAR_COLOR, &clear, 0, 0);
        gs_ortho(0, (float)w, 0, (float)h, -100, 100);
        obs_source_video_render(source);
        gs_texrender_end(target);
        gs_stagesurf_t *stage = gs_stagesurface_create(w,h,GS_RGBA);
        gs_stage_texture(stage, gs_texrender_get_texture(target));
        uint8_t *received; uint32_t stride;
        assert(gs_stagesurface_map(stage,&received,&stride));
        const uint8_t *center = received + (h/2)*stride + (w/2)*4;
        fprintf(stderr,"OBS canvas center RGBA: %u %u %u %u\n",center[0],center[1],center[2],center[3]);
        assert(abs((int)center[0]-64)<=2 && abs((int)center[1]-32)<=2 && abs((int)center[2]-16)<=2 && abs((int)center[3]-128)<=2);
        gs_stagesurface_unmap(stage); gs_stagesurface_destroy(stage); gs_texrender_destroy(target);
        obs_leave_graphics();
        aria_canvas_destroy(canvas);
        for(int wait=0;wait<100 && obs_source_get_width(source);wait++) usleep(30000);
        assert(!obs_source_get_width(source));
    }
    obs_set_output_source(0,NULL); obs_source_release(source); obs_shutdown();
    puts("Native OBS: module load, source discovery, alpha render, resized sender reconnect and close passed.");
}
