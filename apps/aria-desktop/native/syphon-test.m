// Native Metal/Syphon receiver test. No ARIA window, camera or model files.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
void *aria_syphon_create(const char *, const char *, void *, void *);
int aria_syphon_send(void *, void *, int);
void aria_syphon_destroy(void *);

@interface ARIATestServer : NSObject
@property(readonly) NSDictionary *serverDescription;
@property(readonly) BOOL hasClients;
@end
@interface ARIATestSender : NSObject
@property(strong) ARIATestServer *server;
@end
@interface ARIATestClient : NSObject
- (id)initWithServerDescription:(NSDictionary *)description device:(id<MTLDevice>)device
                       options:(NSDictionary *)options newFrameHandler:(void (^)(id))handler;
- (id<MTLTexture>)newFrameImage;
- (void)stop;
@end

// Count only publication command buffers, not the test's readback fences.
@interface ARIACountingQueue : NSObject
@property(strong) id<MTLCommandQueue> queue;
@property NSUInteger submissions;
- (id<MTLCommandBuffer>)commandBuffer;
@end
@implementation ARIACountingQueue
- (id<MTLCommandBuffer>)commandBuffer {
    self.submissions++;
    return [self.queue commandBuffer];
}
@end

static void until(BOOL (^ready)(void), const char *reason) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5.0];
    while (!ready()) {
        if ([deadline timeIntervalSinceNow] <= 0) {
            fprintf(stderr, "Syphon timeout: %s\n", reason);
            abort();
        }
        [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    }
}
static void finish(id<MTLCommandQueue> queue) {
    id<MTLCommandBuffer> fence = [queue commandBuffer];
    [fence commit]; [fence waitUntilCompleted];
    assert(fence.status == MTLCommandBufferStatusCompleted);
}
static void receive(ARIATestClient *client, id<MTLCommandQueue> queue,
                    const uint8_t *expected, NSUInteger w, NSUInteger h) {
    __block id<MTLTexture> shared = nil;
    until(^BOOL {
        shared = [client newFrameImage];
        return shared && shared.width == w && shared.height == h;
    }, "client image/resize");
    assert(shared.pixelFormat == MTLPixelFormatBGRA8Unorm);
    // IOSurface storage can be managed on Intel; synchronize before CPU read.
    if (shared.storageMode == MTLStorageModeManaged) {
        id<MTLCommandBuffer> sync = [queue commandBuffer];
        id<MTLBlitCommandEncoder> blit = [sync blitCommandEncoder];
        [blit synchronizeResource:shared]; [blit endEncoding];
        [sync commit]; [sync waitUntilCompleted];
        assert(sync.status == MTLCommandBufferStatusCompleted);
    }
    uint8_t *pixels = malloc(w*h*4);
    assert(pixels);
    [shared getBytes:pixels bytesPerRow:w*4 fromRegion:MTLRegionMake2D(0,0,w,h) mipmapLevel:0];
    // OBS's Syphon source builds top vertices with V=height and bottom with V=0
    // (plugins/mac-syphon/syphon.m: syphon_video_tick/build_sprite_rect).
    // Emulate that consumer convention, checking every pixel: unique X/Y colors
    // detect vertical inversion, horizontal mirroring and a 180-degree rotation.
    for (NSUInteger y=0; y<h; y++) {
        assert(!memcmp(expected+y*w*4, pixels+(h-1-y)*w*4, w*4));
    }
    free(pixels);
}
int main(int argc, const char **argv) {
    @autoreleasepool {
        assert(argc == 2);
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) { puts("SKIP: runner has no Metal device"); return 77; }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        assert(queue);
        for (int rgba=0; rgba<2; rgba++) {
            ARIACountingQueue *counted = [ARIACountingQueue new];
            counted.queue = queue;
            void *handle = aria_syphon_create(argv[1], "ARIA Alpha orientation CI",
                (__bridge void *)device, (__bridge void *)counted);
            assert(handle);
            ARIATestSender *sender = (__bridge ARIATestSender *)handle;
            ARIATestClient *client = nil;
            for (int shape=0; shape<3; shape++) {
                NSUInteger w = shape == 0 ? 65 : shape == 1 ? 128 : 97;
                NSUInteger h = shape == 0 ? 128 : shape == 1 ? 65 : 83;
                MTLTextureDescriptor *desc = [MTLTextureDescriptor
                    texture2DDescriptorWithPixelFormat:rgba ? MTLPixelFormatRGBA8Unorm : MTLPixelFormatBGRA8Unorm
                    width:w height:h mipmapped:NO];
                desc.storageMode = MTLStorageModeShared;
                desc.usage = MTLTextureUsageShaderRead | MTLTextureUsageRenderTarget;
                id<MTLTexture> source = [device newTextureWithDescriptor:desc];
                assert(source);
                uint8_t *pixels = malloc(w*h*4), *expected = malloc(w*h*4);
                assert(pixels && expected);
                for (NSUInteger y=0; y<h; y++) for (NSUInteger x=0; x<w; x++) {
                    NSUInteger i=(y*w+x)*4;
                    uint8_t bgra[4]={(uint8_t)(x%89+1),(uint8_t)(y%97+2),
                                    (uint8_t)((x*3+y*5)%113+3),160};
                    if (!x && !y) memset(bgra,0,4);
                    memcpy(expected+i,bgra,4);
                    memcpy(pixels+i,bgra,4);
                    if (rgba) { pixels[i]=bgra[2]; pixels[i+2]=bgra[0]; }
                }
                [source replaceRegion:MTLRegionMake2D(0,0,w,h) mipmapLevel:0 withBytes:pixels bytesPerRow:w*4];
                assert(aria_syphon_send(handle, (__bridge void *)source, 1) == 1);
                finish(queue);
                if (!client) {
                    assert(!sender.server.hasClients);
                    // Change the canvas while OBS is absent, then freeze it.
                    expected[4] = 77; pixels[rgba ? 6 : 4] = 77;
                    [source replaceRegion:MTLRegionMake2D(0,0,w,h) mipmapLevel:0 withBytes:pixels bytesPerRow:w*4];
                    NSUInteger before = counted.submissions;
                    assert(aria_syphon_send(handle, (__bridge void *)source, 1) == 1);
                    assert(aria_syphon_send(handle, (__bridge void *)source, 0) == 1);
                    assert(counted.submissions == before);
                    Class api = NSClassFromString(@"SyphonMetalClient");
                    assert(api);
                    client = [[api alloc] initWithServerDescription:sender.server.serverDescription
                        device:device options:nil newFrameHandler:nil];
                    assert(client);
                    until(^BOOL { return sender.server.hasClients; }, "late client connection");
                    assert(aria_syphon_send(handle, (__bridge void *)source, 0) == 1);
                    assert(counted.submissions == before+1);
                    finish(queue);
                }
                receive(client,queue,expected,w,h);
                NSUInteger before = counted.submissions;
                for (int idle=0; idle<120; idle++) {
                    assert(aria_syphon_send(handle, (__bridge void *)source, 0) == 1);
                }
                assert(counted.submissions == before);
                free(pixels); free(expected);
            }
            [client stop];
            until(^BOOL { return !sender.server.hasClients; }, "client disconnect");
            aria_syphon_destroy(handle);
        }
        puts("Syphon client: upright OBS pixel convention, no horizontal mirror, BGRA/RGBA, alpha, portrait/landscape/Freeform resize, late clients and 120 idle frames without GPU submissions passed.");
    }
}
