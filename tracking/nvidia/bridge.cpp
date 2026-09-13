// ARIA's optional C ABI bridge. Build against the user's NVIDIA AR SDK headers.
// API sequencing follows NVIDIA-Maxine/AR-SDK-Samples (MIT); see NOTICE.md.
#include "nvAR.h"
#include "nvARFaceExpressions.h"
#include "nvCVImage.h"
#include <algorithm>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

static thread_local std::string last_error;
static void check(NvCV_Status s) {
    if (s != NVCV_SUCCESS) throw std::runtime_error("NVIDIA AR SDK error " + std::to_string(int(s)) + ": " + NvCV_GetErrorStringFromCode(s));
}
struct Tracker {
    NvAR_FeatureHandle feature{};
    CUstream stream{};
    NvCVImage cpu{}, gpu{};
    NvAR_Quaternion pose{};
    NvAR_Vector3f translation{};
    NvAR_Rect box_data[25]{};
    NvAR_BBoxes boxes{};
    std::vector<NvAR_Point2f> landmarks;
    std::vector<NvAR_Point3f> landmarks3d;
    std::vector<float> confidence, expressions;
    float intrinsic[3]{};
    ~Tracker() {
        if(feature) NvAR_Destroy(feature);
        NvCVImage_Dealloc(&gpu); NvCVImage_Dealloc(&cpu);
        if(stream) NvAR_CudaStreamDestroy(stream);
    }
    void init(unsigned width, unsigned height, const char* model_dir) {
        check(NvAR_CudaStreamCreate(&stream));
        check(NvCVImage_Alloc(&cpu,width,height,NVCV_BGR,NVCV_U8,NVCV_CHUNKY,NVCV_CPU,1));
        check(NvCVImage_Alloc(&gpu,width,height,NVCV_BGR,NVCV_U8,NVCV_CHUNKY,NVCV_GPU,1));
        check(NvAR_Create(NvAR_Feature_FaceExpressions,&feature));
        if(model_dir && *model_dir) check(NvAR_SetString(feature,NvAR_Parameter_Config(ModelDir),model_dir));
        check(NvAR_SetCudaStream(feature,NvAR_Parameter_Config(CUDAStream),stream));
        check(NvAR_SetU32(feature,NvAR_Parameter_Config(Temporal),55));
        check(NvAR_SetU32(feature,NvAR_Parameter_Config(PoseMode),0));
        check(NvAR_Load(feature));
        unsigned count=0, points=0;
        check(NvAR_GetU32(feature,NvAR_Parameter_Config(ExpressionCount),&count));
        check(NvAR_GetU32(feature,NvAR_Parameter_Config(Landmarks_Size),&points));
        if(count!=53 || points==0 || points>512) throw std::runtime_error("Unsupported NVIDIA expression/landmark layout. This bridge requires the 53-coefficient FaceExpressions feature.");
        expressions.resize(count); landmarks.resize(points); landmarks3d.resize(points); confidence.resize(points);
        boxes.boxes=box_data; boxes.max_boxes=25; boxes.num_boxes=0;
        check(NvAR_SetObject(feature,NvAR_Parameter_Output(BoundingBoxes),&boxes,sizeof(boxes)));
        check(NvAR_SetObject(feature,NvAR_Parameter_Output(Landmarks),landmarks.data(),sizeof(NvAR_Point2f)));
        check(NvAR_SetObject(feature,NvAR_Parameter_Output(Landmarks3d),landmarks3d.data(),sizeof(NvAR_Point3f)));
        check(NvAR_SetF32Array(feature,NvAR_Parameter_Output(LandmarksConfidence),confidence.data(),points));
        check(NvAR_SetF32Array(feature,NvAR_Parameter_Output(ExpressionCoefficients),expressions.data(),count));
        check(NvAR_SetObject(feature,NvAR_Parameter_Output(Pose),&pose,sizeof(pose)));
        check(NvAR_SetObject(feature,NvAR_Parameter_Output(PoseTranslation),&translation,sizeof(translation)));
        intrinsic[0]=float(height);intrinsic[1]=float(width)/2;intrinsic[2]=float(height)/2;
        check(NvAR_SetF32Array(feature,NvAR_Parameter_Input(CameraIntrinsicParams),intrinsic,3));
        check(NvAR_SetObject(feature,NvAR_Parameter_Input(Image),&gpu,sizeof(gpu)));
    }
};
#define API extern "C" __declspec(dllexport)
API const char* aria_nvar_error() {return last_error.c_str();}
API void* aria_nvar_create(unsigned w,unsigned h,const char* model_dir) {
    try {if(w<160||h<120||w>1920||h>1080)throw std::runtime_error("Unsupported frame size");auto t=std::make_unique<Tracker>();t->init(w,h,model_dir);return t.release();}
    catch(const std::exception& e){last_error=e.what();return nullptr;}
}
API void aria_nvar_destroy(void* handle){delete static_cast<Tracker*>(handle);}
// output: quaternion x,y,z,w followed by the SDK's 53 expression coefficients.
API int aria_nvar_track(void* handle,const unsigned char* pixels,unsigned w,unsigned h,unsigned pitch,float* output,unsigned capacity) {
    try {
        auto t=static_cast<Tracker*>(handle);
        if(!t||!pixels||!output||capacity<57||w!=t->cpu.width||h!=t->cpu.height||pitch<w*3)throw std::runtime_error("Invalid frame buffer");
        for(unsigned y=0;y<h;++y)std::memcpy(static_cast<unsigned char*>(t->cpu.pixels)+y*t->cpu.pitch,pixels+y*pitch,w*3);
        t->boxes.num_boxes=0;
        std::fill(t->expressions.begin(),t->expressions.end(),0.f);
        check(NvCVImage_Transfer(&t->cpu,&t->gpu,1.f,t->stream,nullptr));
        check(NvAR_Run(t->feature));
        if(t->boxes.num_boxes==0)return 0;
        output[0]=t->pose.x;output[1]=t->pose.y;output[2]=t->pose.z;output[3]=t->pose.w;
        std::copy(t->expressions.begin(),t->expressions.end(),output+4);
        return 1;
    } catch(const std::exception& e){last_error=e.what();return -1;}
}
