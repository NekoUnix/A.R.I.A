// Native Metal/Syphon test; does not start ARIA or access camera/model files.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <assert.h>
#include <stdio.h>
void *aria_syphon_create(const char *, const char *, void *, void *);
int aria_syphon_send(void *, void *);
void aria_syphon_destroy(void *);
// Internal state used only by the native bridge integration test.
@interface ARIATestAPI : NSObject
- (id<MTLTexture>)newFrameImage;
@end
@interface ARIATestSender : NSObject
@property(strong) ARIATestAPI *server;
@end
int main(int argc, const char **argv) {
    @autoreleasepool {
        assert(argc == 2);
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) { puts("SKIP: runner has no Metal device"); return 77; }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        for (int shape = 0; shape < 2; shape++) {
            NSUInteger w = shape ? 128 : 65, h = shape ? 65 : 128;
            MTLTextureDescriptor *desc = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm width:w height:h mipmapped:NO];
            desc.storageMode = MTLStorageModeShared;
            desc.usage = MTLTextureUsageShaderRead | MTLTextureUsageRenderTarget;
            id<MTLTexture> source = [device newTextureWithDescriptor:desc];
            assert(source);
            uint8_t *pixels = malloc(w*h*4), *received = malloc(w*h*4);
            for (NSUInteger i=0; i<w*h; i++) { pixels[i*4]=(uint8_t)(i%128); pixels[i*4+1]=32; pixels[i*4+2]=64; pixels[i*4+3]=128; }
            [source replaceRegion:MTLRegionMake2D(0,0,w,h) mipmapLevel:0 withBytes:pixels bytesPerRow:w*4];
            void *handle = aria_syphon_create(argv[1], "ARIA Alpha CI", (__bridge void *)device, (__bridge void *)queue);
            assert(handle);
            assert(aria_syphon_send(handle, (__bridge void *)source) == 1);
            id<MTLCommandBuffer> fence = [queue commandBuffer]; [fence commit]; [fence waitUntilCompleted];
            ARIATestSender *sender = (__bridge ARIATestSender *)handle;
            id<MTLTexture> shared = [sender.server newFrameImage];
            assert(shared && shared.width == w && shared.height == h);
            // IOSurface storage can be managed on Intel; synchronize before CPU read.
            if (shared.storageMode == MTLStorageModeManaged) {
                id<MTLCommandBuffer> sync = [queue commandBuffer];
                id<MTLBlitCommandEncoder> blit = [sync blitCommandEncoder];
                [blit synchronizeResource:shared]; [blit endEncoding]; [sync commit]; [sync waitUntilCompleted];
            }
            [shared getBytes:received bytesPerRow:w*4 fromRegion:MTLRegionMake2D(0,0,w,h) mipmapLevel:0];
            assert(!memcmp(pixels, received, w*h*4));
            aria_syphon_destroy(handle); free(pixels); free(received);
        }
        puts("Syphon Metal: portrait/landscape, channel order, alpha pixels and sender cleanup passed.");
    }
}
