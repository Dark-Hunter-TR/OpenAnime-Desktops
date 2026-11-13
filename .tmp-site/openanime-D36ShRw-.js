/*
* OpenAnime
* https://openani.me
* (c) 2026 OpenAnime Labs.
* Licensed under the BSL-1.1 License
* More information at https://github.com/OpenAnime/licenses/blob/main/BSL.txt
*/

/*!
 ******************* BEGIN OpenFrameGeneration 3.5 Copyright Notice ****************
 *
 * COPYRIGHT
 *
 * Copyright (c) 2025-2026, RAISE-SR Visual Computing Group, OpenAnime Labs.
 * All rights reserved.
 * 
 *
 ***************** END OpenFrameGeneration 3.5 Copyright Notice ********************
 */

import{s as b,l,w as _}from"./openanime-sATobStx.js";import{S as h,i as w,b as g,c as C,d as I,m as A,t as P,a as B,f as D}from"./openanime-D6JUTAw9.js";import{e as S}from"./openanime-LfDqg_SJ.js";import{I as U}from"./openanime-DAUN7vEt.js";import{t as j}from"./openanime-DNF7RXUm.js";function q(o){let n,a,t,i;function s(e){o[4](e)}function c(e){o[5](e)}let f={aspect:1,disabled:o[1],cropOnBackend:!0};return o[0]!==void 0&&(f.open=o[0]),o[2]!==void 0&&(f.error=o[2]),n=new U({props:f}),l.push(()=>g(n,"open",s)),l.push(()=>g(n,"error",c)),n.$on("file",o[3]),{c(){C(n.$$.fragment)},l(e){I(n.$$.fragment,e)},m(e,r){A(n,e,r),i=!0},p(e,[r]){const p={};r&2&&(p.disabled=e[1]),!a&&r&1&&(a=!0,p.open=e[0],_(()=>a=!1)),!t&&r&4&&(t=!0,p.error=e[2],_(()=>t=!1)),n.$set(p)},i(e){i||(P(n.$$.fragment,e),i=!0)},o(e){B(n.$$.fragment,e),i=!1},d(e){D(n,e)}}}function v(o,n,a){let{open:t=!1}=n,i,s="";async function c(r){const{fileUpload:p,pixelCrop:d}=r.detail;a(1,i=!0);const m=new FormData;m.append("avatar",p),m.append("cropData",JSON.stringify(d));const u=await fetch(`${S.PUBLIC_API_LINK}/user/avatar`,{method:"PUT",headers:{Authorization:window.getCookie("token")},body:m});if(a(1,i=!1),u.ok)a(0,t=!1);else{const k=await u.json();a(2,s=j(k.error))}}function f(r){t=r,a(0,t)}function e(r){s=r,a(2,s)}return o.$$set=r=>{"open"in r&&a(0,t=r.open)},[t,i,s,c,f,e]}class F extends h{constructor(n){super(),w(this,n,v,q,b,{open:0})}}export{F as C};
