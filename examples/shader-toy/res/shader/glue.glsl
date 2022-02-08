#version 460 core

layout(push_constant) uniform PushConstants {
    layout(offset = 0)  uniform vec3      iResolution;           // viewport resolution (in pixels)
    layout(offset = 16) uniform vec4      iDate;                 // (year, month, day, time in seconds)
    layout(offset = 32) uniform vec4      iMouse;                // mouse pixel coords. xy: current (if MLB down), zw: click
    layout(offset = 48) uniform float     iTime;                 // shader playback time (in seconds)
    layout(offset = 52) uniform float     iTimeDelta;            // render time (in seconds)
    layout(offset = 56) uniform int       iFrame;                // shader playback frame
    layout(offset = 60) uniform float     iSampleRate;           // sound sample rate (i.e., 44100)
    layout(offset = 64) uniform float     iChannelTime[4];       // channel playback time (in seconds)
    layout(offset = 80) uniform vec3      iChannelResolution[4]; // channel resolution (in pixels)
};

layout(set = 0, binding = 0) uniform sampler2D iChannel0;             // input channel. XX = 2D/Cube
layout(set = 0, binding = 1) uniform sampler2D iChannel1;             // input channel. XX = 2D/Cube
layout(set = 0, binding = 2) uniform sampler2D iChannel2;             // input channel. XX = 2D/Cube
layout(set = 0, binding = 3) uniform sampler2D iChannel3;             // input channel. XX = 2D/Cube

layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;