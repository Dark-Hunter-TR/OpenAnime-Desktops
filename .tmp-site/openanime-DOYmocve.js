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

import{s as E,m as c,e as I,n as q,a as B,b as S,o as T,d as g,q as p,r as v,i as j,v as A,x as h,A as C,c as D,u as P,j as V,k as x}from"./openanime-sATobStx.js";import{S as z,i as F,c as $,d as y,m as b,t as m,a as u,f as k}from"./openanime-D6JUTAw9.js";import{T as G,g as H}from"./openanime-LfDqg_SJ.js";import{I as J}from"./openanime-DdgZ6iY0.js";/* empty css                  */function K(l){let e;const r=l[2].default,a=D(r,l,l[3],null);return{c(){a&&a.c()},l(t){a&&a.l(t)},m(t,n){a&&a.m(t,n),e=!0},p(t,n){a&&a.p&&(!e||n&8)&&P(a,r,t,t[3],e?x(r,t[3],n,null):V(t[3]),null)},i(t){e||(m(a,t),e=!0)},o(t){u(a,t),e=!1},d(t){a&&a.d(t)}}}function L(l){let e,r,a,t,n;r=new J({props:{severity:l[0]}}),t=new G({props:{$$slots:{default:[K]},$$scope:{ctx:l}}});let f=[{class:"error"},l[1]],i={};for(let s=0;s<f.length;s+=1)i=c(i,f[s]);return{c(){e=I("div"),$(r.$$.fragment),a=q(),$(t.$$.fragment),this.h()},l(s){e=B(s,"DIV",{class:!0});var o=S(e);y(r.$$.fragment,o),a=T(o),y(t.$$.fragment,o),o.forEach(g),this.h()},h(){p(e,i),v(e,"svelte-1f8yx29",!0)},m(s,o){j(s,e,o),b(r,e,null),A(e,a),b(t,e,null),n=!0},p(s,[o]){const _={};o&1&&(_.severity=s[0]),r.$set(_);const d={};o&8&&(d.$$scope={dirty:o,ctx:s}),t.$set(d),p(e,i=H(f,[{class:"error"},o&2&&s[1]])),v(e,"svelte-1f8yx29",!0)},i(s){n||(m(r.$$.fragment,s),m(t.$$.fragment,s),n=!0)},o(s){u(r.$$.fragment,s),u(t.$$.fragment,s),n=!1},d(s){s&&g(e),k(r),k(t)}}}function M(l,e,r){const a=["severity"];let t=h(e,a),{$$slots:n={},$$scope:f}=e,{severity:i="critical"}=e;return l.$$set=s=>{e=c(c({},e),C(s)),r(1,t=h(e,a)),"severity"in s&&r(0,i=s.severity),"$$scope"in s&&r(3,f=s.$$scope)},[i,t,n,f]}class W extends z{constructor(e){super(),F(this,e,M,L,E,{severity:0})}}export{W as E};
