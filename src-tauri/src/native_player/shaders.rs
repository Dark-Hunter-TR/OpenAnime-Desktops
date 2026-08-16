// ═══════════════════════════════════════════════════════════
// WGSL Shader Kaynakları — openani.me oynatıcısından BİREBİR yakalandı
// ═══════════════════════════════════════════════════════════
//
// Bu kaynaklar webgpu-inspector.js ile sitenin createShaderModule çağrılarından
// yakalandı. Native wgpu aynı WGSL'i derlediği için hiçbir değişiklik gerekmez.
//
// Sitede 5 shader var; ikisi `texture_external` (tarayıcı video importu) kullanır
// ve native wgpu'da YOKTUR. Bu yüzden yalnızca `texture_2d` varyantları
// kullanılır:
//   DISPLAY_SHADER — görüntü (LUT renk derecelendirme + FRC dithering)  [shader #0]
//   SCALE_SHADER   — yeniden boyutlandırma + sharpen                    [shader #3]
//
// texture_external varyantları (#1, #2, #4) tarayıcıya özgü olduğu için burada
// kullanılmaz; biz kareyi texture_2d'ye kendimiz yükleriz.

/// Shader #0 — ana görüntü shader'ı.
/// Bindings (group 0): 0=sampler, 1=imageTexture(texture_2d), 2=frc(uniform),
/// 3=filterImageTexture(texture_2d, 512×512 64³ 3D LUT).
pub const DISPLAY_SHADER: &str = r#"struct VertexOut{@builtin(position)position:vec4<f32>,@location(1)texelCoords:vec2<f32>}@group(0)@binding(0)var linearSampler:sampler;@group(0)@binding(1)var imageTexture:texture_2d<f32>;@group(0)@binding(3)var filterImageTexture:texture_2d<f32>;struct FrcUniforms{frameCount:u32,frcEnabled:u32,};@group(0)@binding(2)var<uniform>frc:FrcUniforms;const bayer_matrix=mat4x4<f32>(vec4<f32>(0.0,8.0,2.0,10.0),vec4<f32>(12.0,4.0,14.0,6.0),vec4<f32>(3.0,11.0,1.0,9.0),vec4<f32>(15.0,7.0,13.0,5.0))*(1.0/16.0);fn c(d:f32,e:vec2<f32>)->f32{let f:f32=255.0;let h=d*f;let i=floor(h);let j=ceil(h);let k=fract(h);let l=i32(e.x)%4;let m=i32(e.y)%4;var n=bayer_matrix[l][m];if((frc.frameCount%2u)==1u){n=1.0-n;}var o=i;if(k>n){o=j;}return o/f;}@vertex fn vertex_main(@location(0)position:vec4<f32>,@location(1)p:vec2<f32>)->VertexOut{var q:VertexOut;q.position=position;q.texelCoords=p;q.texelCoords.y=1.0-q.texelCoords.y;return q;}@fragment fn fragment_main(s:VertexOut)->@location(0)vec4<f32>{var t=textureSample(imageTexture,linearSampler,s.texelCoords).rgba;t=clamp(t,vec4<f32>(0.0,0.0,0.0,0.0),vec4<f32>(1.0,1.0,1.0,1.0));var u=t.b*63.0;var v:vec2<f32>;v.y=floor(floor(u)*0.125);v.x=floor(u)-(v.y*8.0);var w:vec2<f32>;w.y=floor(ceil(u)*0.125);w.x=ceil(u)-(w.y*8.0);var z:vec2<f32>;z.x=((v.x*64.0)+t.r*63.0+0.5)/512.0;z.y=((v.y*64.0)+t.g*63.0+0.5)/512.0;var aa:vec2<f32>;aa.x=((w.x*64.0)+t.r*63.0+0.5)/512.0;aa.y=((w.y*64.0)+t.g*63.0+0.5)/512.0;var ab=textureSample(filterImageTexture,linearSampler,z);var ac=textureSample(filterImageTexture,linearSampler,aa);let ad=mix(ab,ac,fract(u));if(frc.frcEnabled==1u){let ae=c(ad.r,s.position.xy);let af=c(ad.g,s.position.xy);let ag=c(ad.b,s.position.xy);return vec4<f32>(ae,af,ag,ad.a);}else{return ad;}}"#;

/// Shader #3 — yeniden boyutlandırma + sharpen (9-tap ağırlıklı yeniden örnekleme).
/// Bindings (group 0): 0=sampler, 1=sourceTexture(texture_2d), 2=scaleInfo(uniform).
pub const SCALE_SHADER: &str = r#"struct ScaleUniforms{sourceSize:vec2<f32>,outputSize:vec2<f32>,};struct VertexOut{@builtin(position)position:vec4<f32>,@location(0)uv:vec2<f32>,};@group(0)@binding(0)var linearSampler:sampler;@group(0)@binding(1)var sourceTexture:texture_2d<f32>;@group(0)@binding(2)var<uniform>scaleInfo:ScaleUniforms;@vertex fn vertex_main(@location(0)position:vec4<f32>,@location(1)b:vec2<f32>)->VertexOut{var e:VertexOut;e.position=position;e.uv=vec2<f32>(b.x,1.0-b.y);return e;}fn a(f:vec2<f32>)->vec4<f32>{let g=max(scaleInfo.sourceSize,vec2<f32>(1.0,1.0));let h=max(scaleInfo.outputSize,vec2<f32>(1.0,1.0));let i=g/h;if(max(i.x,i.y)<=1.15){return textureSampleLevel(sourceTexture,linearSampler,f,0.0);}let j=vec2<f32>(1.0,1.0)/g;let k=clamp((i-vec2<f32>(1.0,1.0))*0.5,vec2<f32>(0.5,0.5),vec2<f32>(2.0,2.0))*j;let m=textureSampleLevel(sourceTexture,linearSampler,f,0.0);let n=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(-k.x,0.0),0.0);let o=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(k.x,0.0),0.0);let p=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(0.0,-k.y),0.0);let q=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(0.0,k.y),0.0);let s=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(-k.x,-k.y),0.0);let t=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(k.x,-k.y),0.0);let v=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(-k.x,k.y),0.0);let w=textureSampleLevel(sourceTexture,linearSampler,f+vec2<f32>(k.x,k.y),0.0);return(m*4.0+(n+o+p+q)*2.0+s+t+v+w)*(1.0/16.0);}@fragment fn fragment_main(z:VertexOut)->@location(0)vec4<f32>{return a(z.uv);}}"#;
