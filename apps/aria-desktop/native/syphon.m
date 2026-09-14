// Original ARIA adapter. Syphon is dynamically loaded from the app bundle.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <stdatomic.h>

@interface ARIASyphonAPI : NSObject
- (id)initWithName:(NSString *)name device:(id<MTLDevice>)device options:(NSDictionary *)options;
- (void)publishFrameTexture:(id<MTLTexture>)texture onCommandBuffer:(id<MTLCommandBuffer>)buffer imageRegion:(NSRect)region flipped:(BOOL)flipped;
- (void)stop;
@property(readonly) BOOL hasClients;
@end
@interface ARIASender : NSObject {
@public atomic_int pending;
}
@property(strong) ARIASyphonAPI *server;
@property(strong) id<MTLCommandQueue> queue;
@property BOOL published;
@property BOOL dirty;
@end
@implementation ARIASender
- (void)dealloc { [_server stop]; }
@end

void *aria_syphon_create(const char *framework, const char *name, void *device, void *queue) {
    @autoreleasepool {
        NSBundle *bundle = [NSBundle bundleWithPath:[NSString stringWithUTF8String:framework]];
        if (![bundle load]) return NULL;
        Class api = NSClassFromString(@"SyphonMetalServer");
        if (!api) return NULL;
        ARIASender *sender = [ARIASender new];
        sender.server = [[api alloc] initWithName:[NSString stringWithUTF8String:name]
                         device:(__bridge id<MTLDevice>)device options:nil];
        if (!sender.server) return NULL;
        sender.queue = (__bridge id<MTLCommandQueue>)queue;
        atomic_init(&sender->pending, 0);
        return (__bridge_retained void *)sender;
    }
}
int aria_syphon_send(void *handle, void *texture, int changed) {
    @autoreleasepool {
        ARIASender *sender = (__bridge ARIASender *)handle;
        sender.dirty = sender.dirty || changed || !sender.published;
        // A frozen/unchanged canvas already has a valid shared frame. Keep a
        // pending change when nobody is listening, so a later client receives
        // the latest canvas even if it stops changing before OBS connects.
        if (!sender.dirty) return 1;
        if (sender.published && !sender.server.hasClients) return 1;
        // Bound GPU work; never wait for OBS or the GPU on the event thread.
        if (atomic_load(&sender->pending) >= 3) return 0;
        id<MTLCommandBuffer> commands = [sender.queue commandBuffer];
        if (!commands) return -1;
        id<MTLTexture> source = (__bridge id<MTLTexture>)texture;
        atomic_fetch_add(&sender->pending, 1);
        [sender.server publishFrameTexture:source onCommandBuffer:commands
            // wgpu renders top-left first. Syphon/OBS consume a bottom-left
            // image: flip vertically here, preserving left/right and alpha.
            // No CPU readback or additional ARIA intermediate texture.
            imageRegion:NSMakeRect(0, 0, source.width, source.height) flipped:YES];
        [commands addCompletedHandler:^(id<MTLCommandBuffer> buffer) {
            (void)buffer;
            atomic_fetch_sub(&sender->pending, 1);
        }];
        // This is wgpu's queue: canvas rendering, sharing, and subsequent renders
        // stay ordered without CPU waits or a second Metal device.
        [commands commit];
        sender.published = YES;
        sender.dirty = NO;
        return 1;
    }
}
void aria_syphon_destroy(void *handle) {
    @autoreleasepool {
        ARIASender *sender = (__bridge_transfer ARIASender *)handle;
        [sender.server stop];
    }
}
